//! Surgical editor for THE FINALS' `EmbarkOptionSaveGame.sav` (Unreal GVAS, UE 5.5+ property tags).
//!
//! The file holds one `Properties` map of name -> string. This module parses only as far as that map,
//! keeps every other byte exactly as it was (`prefix`, `suffix`) and keeps each untouched entry's original
//! bytes, so a file that isn't edited serialises back byte-for-byte. Only edited entries are re-encoded,
//! and the map's size field is patched to match.

use std::fmt;

#[derive(Debug)]
pub struct GvasError(pub String);

impl fmt::Display for GvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

type Res<T> = Result<T, GvasError>;

fn err<T>(msg: impl Into<String>) -> Res<T> {
    Err(GvasError(msg.into()))
}

pub struct Entry {
    pub key: String,
    pub value: String,
    raw_key: Vec<u8>,
    raw_val: Vec<u8>,
    dirty: bool,
}

pub struct SaveFile {
    pub class_name: String,
    /// The save's own `Version` integer property (8 at the time of writing), if present.
    pub version: Option<i32>,
    prefix: Vec<u8>,
    size_pos: usize,
    entries: Vec<Entry>,
    suffix: Vec<u8>,
}

struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cur<'a> {
    fn need(&self, n: usize) -> Res<()> {
        if self.i + n > self.b.len() {
            return err("file ends unexpectedly");
        }
        Ok(())
    }
    fn u8(&mut self) -> Res<u8> {
        self.need(1)?;
        let v = self.b[self.i];
        self.i += 1;
        Ok(v)
    }
    fn i32(&mut self) -> Res<i32> {
        self.need(4)?;
        let v = i32::from_le_bytes(self.b[self.i..self.i + 4].try_into().unwrap());
        self.i += 4;
        Ok(v)
    }
    fn skip(&mut self, n: usize) -> Res<()> {
        self.need(n)?;
        self.i += n;
        Ok(())
    }
    /// Unreal FString: i32 length (negative = UTF-16), null terminated.
    fn fstring(&mut self) -> Res<String> {
        let n = self.i32()?;
        if n == 0 {
            return Ok(String::new());
        }
        if n > 0 {
            let n = n as usize;
            self.need(n)?;
            let raw = &self.b[self.i..self.i + n - 1];
            self.i += n;
            Ok(String::from_utf8_lossy(raw).into_owned())
        } else {
            let units = (-(n as i64)) as usize;
            self.need(units * 2)?;
            let u: Vec<u16> = self.b[self.i..self.i + (units - 1) * 2]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            self.i += units * 2;
            Ok(String::from_utf16_lossy(&u))
        }
    }
    /// FPropertyTypeName: name, inner count, inner type names.
    fn type_name(&mut self) -> Res<(String, Vec<(String, usize)>)> {
        let name = self.fstring()?;
        let n = self.i32()?;
        if !(0..=8).contains(&n) {
            return err("unsupported property type layout");
        }
        let mut inner = Vec::new();
        for _ in 0..n {
            let (t, sub) = self.type_name()?;
            inner.push((t, sub.len()));
        }
        Ok((name, inner))
    }
}

pub fn encode_fstring(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    if s.is_empty() {
        out.extend_from_slice(&0i32.to_le_bytes());
    } else if s.is_ascii() {
        out.extend_from_slice(&((s.len() + 1) as i32).to_le_bytes());
        out.extend_from_slice(s.as_bytes());
        out.push(0);
    } else {
        let units: Vec<u16> = s.encode_utf16().collect();
        out.extend_from_slice(&(-((units.len() + 1) as i32)).to_le_bytes());
        for u in units {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out.extend_from_slice(&[0, 0]);
    }
    out
}

impl SaveFile {
    pub fn parse(bytes: &[u8]) -> Res<SaveFile> {
        let mut c = Cur { b: bytes, i: 0 };
        if bytes.len() < 4 || &bytes[..4] != b"GVAS" {
            return err("not an Unreal save file");
        }
        c.i = 4;
        c.skip(12)?; // save version, UE4 package version, UE5 package version
        c.skip(10)?; // engine major/minor/patch (u16 each) + changelist
        c.fstring()?; // engine branch
        c.skip(4)?; // custom version format
        let n = c.i32()?;
        if !(0..=4096).contains(&n) {
            return err("unexpected custom version table");
        }
        c.skip(n as usize * 20)?;
        let class_name = c.fstring()?;
        c.u8()?; // class serialisation extension byte

        let mut version = None;
        loop {
            let name = c.fstring()?;
            if name == "None" {
                return err("no Properties map in this file");
            }
            let (ty, inner) = c.type_name()?;
            let size_pos = c.i;
            let size = c.i32()? as usize;
            let flags = c.u8()?;
            if flags & 1 != 0 {
                c.skip(4)?;
            }
            if flags & 2 != 0 {
                c.skip(16)?;
            }
            let data_start = c.i;
            if ty == "MapProperty" && name == "Properties" {
                if inner.len() != 2 || inner[0].0 != "NameProperty" || inner[1].0 != "StrProperty" {
                    return err("the Properties map has an unexpected layout");
                }
                let removed = c.i32()?;
                if removed != 0 {
                    return err("the Properties map has removed entries (unsupported)");
                }
                let count = c.i32()?;
                if !(0..=100_000).contains(&count) {
                    return err("implausible number of settings");
                }
                let mut entries = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let k0 = c.i;
                    let key = c.fstring()?;
                    let k1 = c.i;
                    let value = c.fstring()?;
                    let v1 = c.i;
                    entries.push(Entry {
                        key,
                        value,
                        raw_key: bytes[k0..k1].to_vec(),
                        raw_val: bytes[k1..v1].to_vec(),
                        dirty: false,
                    });
                }
                if c.i != data_start + size {
                    return err("the Properties map size doesn't match its contents");
                }
                return Ok(SaveFile {
                    class_name,
                    version,
                    prefix: bytes[..data_start].to_vec(),
                    size_pos,
                    entries,
                    suffix: bytes[c.i..].to_vec(),
                });
            }
            if ty == "IntProperty" && name == "Version" && size == 4 {
                c.need(4)?;
                version = Some(i32::from_le_bytes(bytes[c.i..c.i + 4].try_into().unwrap()));
            }
            c.i = data_start;
            c.skip(size)?;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .map(|e| (e.key.as_str(), e.value.as_str()))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.value.as_str())
    }

    /// Changes an existing setting. Returns false if the file has no such setting (nothing is ever added).
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        match self.entries.iter_mut().find(|e| e.key == key) {
            Some(e) => {
                if e.value != value {
                    e.value = value.to_string();
                    e.dirty = true;
                }
                true
            }
            None => false,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&0i32.to_le_bytes());
        body.extend_from_slice(&(self.entries.len() as i32).to_le_bytes());
        for e in &self.entries {
            if e.dirty {
                body.extend_from_slice(&encode_fstring(&e.key));
                body.extend_from_slice(&encode_fstring(&e.value));
            } else {
                body.extend_from_slice(&e.raw_key);
                body.extend_from_slice(&e.raw_val);
            }
        }
        let mut out = Vec::with_capacity(self.prefix.len() + body.len() + self.suffix.len());
        out.extend_from_slice(&self.prefix);
        out[self.size_pos..self.size_pos + 4].copy_from_slice(&(body.len() as i32).to_le_bytes());
        out.extend_from_slice(&body);
        out.extend_from_slice(&self.suffix);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Option<Vec<u8>> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/opt.sav"
        ))
        .ok()
    }

    #[test]
    fn untouched_file_round_trips_byte_for_byte() {
        let Some(bytes) = fixture() else { return };
        let f = SaveFile::parse(&bytes).expect("parse");
        assert!(f.len() > 100);
        assert_eq!(
            f.to_bytes(),
            bytes,
            "no-op serialisation must equal the original"
        );
    }

    #[test]
    fn editing_one_value_changes_only_that_value() {
        let Some(bytes) = fixture() else { return };
        let mut f = SaveFile::parse(&bytes).unwrap();
        let before: Vec<(String, String)> = f
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert!(f.set("GameplayOption.Crosshair.Center.Radius", "7.5"));
        let out = f.to_bytes();
        let g = SaveFile::parse(&out).expect("re-parse the edited file");
        assert_eq!(g.get("GameplayOption.Crosshair.Center.Radius"), Some("7.5"));
        for (k, v) in &before {
            if k != "GameplayOption.Crosshair.Center.Radius" {
                assert_eq!(g.get(k), Some(v.as_str()), "{k} changed unexpectedly");
            }
        }
        assert_eq!(g.len(), before.len());
        // everything after the map is byte-identical
        assert_eq!(f.suffix, g.suffix);
    }

    #[test]
    fn unknown_keys_are_never_added() {
        let Some(bytes) = fixture() else { return };
        let mut f = SaveFile::parse(&bytes).unwrap();
        let n = f.len();
        assert!(!f.set("GameplayOption.Does.Not.Exist", "1"));
        assert_eq!(f.len(), n);
        assert_eq!(f.to_bytes(), bytes);
    }

    #[test]
    fn non_ascii_values_survive() {
        let Some(bytes) = fixture() else { return };
        let mut f = SaveFile::parse(&bytes).unwrap();
        f.set("GameplayOption.Online.Region", "Zürich★");
        let g = SaveFile::parse(&f.to_bytes()).unwrap();
        assert_eq!(g.get("GameplayOption.Online.Region"), Some("Zürich★"));
    }

    #[test]
    fn reports_the_save_version() {
        let Some(bytes) = fixture() else { return };
        let f = SaveFile::parse(&bytes).unwrap();
        assert_eq!(f.version, Some(8));
        assert!(f.class_name.ends_with("EmbarkOptionSaveGameV1"));
    }

    #[test]
    fn garbage_is_rejected_not_panicked() {
        assert!(SaveFile::parse(b"hello").is_err());
        let Some(bytes) = fixture() else { return };
        assert!(SaveFile::parse(&bytes[..bytes.len() / 2]).is_err());
    }
}
