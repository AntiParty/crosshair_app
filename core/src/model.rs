//! Crosshair settings as the game stores them: a flat map of `GameplayOption.*` -> string, restricted to the
//! crosshair and sight keys. Typed access goes through the game's own ranges (`schema_data`), so values are
//! always clamped to what the in-game menu allows. Unknown keys are never produced.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::schema_data::{BOOLS, FLOATS};

pub const CENTER_SHAPES: &[&str] = &["Dot", "Circle"];
pub const RETICLE_SHAPES: &[&str] = &["Plus", "Circle", "TShape"];
pub const SIGHT_SHAPES: &[&str] = &["Dot", "Circle", "Cross", "TShape", "Triangle", "XShape"];
/// A real preset is about 5 KB; anything beyond this is refused before it is parsed.
pub const MAX_CODE_BYTES: usize = 16 * 1024;

pub const MELEE_LOOKS: &[&str] = &["Same", "Custom"];

const ENUMS: &[(&str, &str, &[&str])] = &[
    (
        "GameplayOption.Crosshair.Center.Shape",
        "ECrosshairCenterShape",
        CENTER_SHAPES,
    ),
    (
        "GameplayOption.Crosshair.MeleeCenter.Shape",
        "ECrosshairCenterShape",
        CENTER_SHAPES,
    ),
    (
        "GameplayOption.Crosshair.StaticReticle.Shape",
        "ECrosshairReticleShape",
        RETICLE_SHAPES,
    ),
    (
        "GameplayOption.Crosshair.DispersionReticle.Shape",
        "ECrosshairReticleShape",
        RETICLE_SHAPES,
    ),
    (
        "GameplayOption.Crosshair.MeleeLook",
        "EMeleeCrosshairLook",
        MELEE_LOOKS,
    ),
    (
        "GameplayOption.Sights.ReticleShape",
        "EReticleShape",
        SIGHT_SHAPES,
    ),
];

pub fn float_spec(key: &str) -> Option<(f32, f32, f32)> {
    FLOATS.iter().find(|f| f.0 == key).map(|f| (f.1, f.2, f.3))
}

fn enum_spec(key: &str) -> Option<(&'static str, &'static [&'static str])> {
    ENUMS.iter().find(|e| e.0 == key).map(|e| (e.1, e.2))
}

/// Every key this app may read or write.
#[cfg(test)]
pub fn all_keys() -> impl Iterator<Item = &'static str> {
    FLOATS
        .iter()
        .map(|f| f.0)
        .chain(BOOLS.iter().map(|b| b.0))
        .chain(ENUMS.iter().map(|e| e.0))
}

fn fmt_float(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    format!("{:?}", r) // 3.0 -> "3.0", 0.96 -> "0.96": the shape the game writes
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Crosshair {
    pub name: String,
    pub values: BTreeMap<String, String>,
}

impl Crosshair {
    /// The game's defaults for every key.
    pub fn defaults() -> Crosshair {
        let mut values = BTreeMap::new();
        for f in FLOATS {
            values.insert(f.0.to_string(), fmt_float(f.1));
        }
        for b in BOOLS {
            values.insert(b.0.to_string(), b.1.to_string());
        }
        for e in ENUMS {
            let first = if e.0.ends_with("MeleeLook") {
                "Same"
            } else {
                e.2[0]
            };
            values.insert(e.0.to_string(), format!("{}::{}", e.1, first));
        }
        Crosshair {
            name: "Default".into(),
            values,
        }
    }

    /// Reads the crosshair keys from a settings iterator; keys the file lacks fall back to defaults.
    pub fn from_settings<'a>(
        name: &str,
        it: impl Iterator<Item = (&'a str, &'a str)>,
    ) -> Crosshair {
        let mut c = Crosshair::defaults();
        c.name = name.to_string();
        for (k, v) in it {
            if c.values.contains_key(k) {
                c.values.insert(k.to_string(), v.to_string());
            }
        }
        c
    }

    pub fn float(&self, key: &str) -> f32 {
        let (d, lo, hi) = float_spec(key).unwrap_or((0.0, f32::MIN, f32::MAX));
        self.values
            .get(key)
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(d)
            .clamp(lo, hi)
    }

    pub fn set_float(&mut self, key: &str, v: f32) {
        if let Some((_, lo, hi)) = float_spec(key) {
            self.values
                .insert(key.to_string(), fmt_float(v.clamp(lo, hi)));
        }
    }

    pub fn flag(&self, key: &str) -> bool {
        self.values.get(key).map(|v| v == "true").unwrap_or(false)
    }

    pub fn set_flag(&mut self, key: &str, v: bool) {
        if BOOLS.iter().any(|b| b.0 == key) {
            self.values.insert(key.to_string(), v.to_string());
        }
    }

    /// Enum value without its type prefix ("Dot" for "ECrosshairCenterShape::Dot").
    pub fn choice(&self, key: &str) -> String {
        self.values
            .get(key)
            .map(|v| v.rsplit("::").next().unwrap_or(v).to_string())
            .unwrap_or_default()
    }

    pub fn set_choice(&mut self, key: &str, v: &str) {
        if let Some((ty, allowed)) = enum_spec(key) {
            if allowed.contains(&v) {
                self.values.insert(key.to_string(), format!("{ty}::{v}"));
            }
        }
    }

    /// RGB (0..255) from `<prefix>.Red/Green/Blue`.
    pub fn rgb(&self, prefix: &str) -> [u8; 3] {
        let ch = |c: &str| {
            self.float(&format!("{prefix}.{c}"))
                .round()
                .clamp(0.0, 255.0) as u8
        };
        [ch("Red"), ch("Green"), ch("Blue")]
    }

    pub fn set_rgb(&mut self, prefix: &str, rgb: [u8; 3]) {
        for (c, v) in ["Red", "Green", "Blue"].iter().zip(rgb) {
            self.set_float(&format!("{prefix}.{c}"), v as f32);
        }
    }

    /// Only the settings that differ from the game's defaults. Anything missing means "default", which is how
    /// the website sends crosshairs to the browser without repeating 80 unchanged values per card.
    pub fn non_default_values(&self) -> BTreeMap<String, String> {
        let d = Crosshair::defaults();
        self.values
            .iter()
            .filter(|(k, v)| d.values.get(*k) != Some(*v))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// The defaults as compact JSON, for the browser to merge `non_default_values` onto.
    pub fn defaults_json() -> String {
        serde_json::to_string(&Crosshair::defaults().values).unwrap_or_default()
    }

    pub fn to_share_code(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// The same crosshair as a much shorter code: only the settings that differ from the game's defaults.
    /// `from_share_code` fills the rest in, so it round-trips.
    pub fn to_short_code(&self) -> String {
        Crosshair {
            name: self.name.clone(),
            values: self.non_default_values(),
        }
        .to_share_code()
    }

    /// Parses and validates pasted or loaded preset text. Everything is checked against the game's schema;
    /// the first problem rejects the whole preset so nothing half-valid can reach the game's file.
    pub fn from_share_code(text: &str) -> Result<Crosshair, String> {
        if text.len() > MAX_CODE_BYTES {
            return Err("That code is too long to be a crosshair preset.".to_string());
        }
        let raw: Crosshair = serde_json::from_str(text.trim())
            .map_err(|_| "That isn't a crosshair preset.".to_string())?;
        let mut out = Crosshair::defaults();
        out.name = raw.name.trim().chars().take(40).collect();
        if out.name.is_empty() {
            out.name = "Imported".into();
        }
        for (k, v) in &raw.values {
            if let Some((_, lo, hi)) = float_spec(k) {
                let f: f32 = v
                    .parse()
                    .map_err(|_| format!("{k}: '{v}' isn't a number"))?;
                if !f.is_finite() || f < lo - 0.001 || f > hi + 0.001 {
                    return Err(format!("{k}: {v} is outside {lo}..{hi}"));
                }
                out.values.insert(k.clone(), fmt_float(f));
            } else if BOOLS.iter().any(|b| b.0 == k) {
                if v != "true" && v != "false" {
                    return Err(format!("{k}: '{v}' should be true or false"));
                }
                out.values.insert(k.clone(), v.clone());
            } else if let Some((ty, allowed)) = enum_spec(k) {
                let short = v.rsplit("::").next().unwrap_or(v);
                if !allowed.contains(&short) {
                    return Err(format!("{k}: '{v}' isn't a valid choice"));
                }
                out.values.insert(k.clone(), format!("{ty}::{short}"));
            } else {
                return Err(format!("'{k}' isn't a crosshair setting"));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_every_key() {
        let d = Crosshair::defaults();
        assert_eq!(d.values.len(), all_keys().count());
    }

    #[test]
    fn floats_are_clamped_and_formatted_like_the_game() {
        let mut c = Crosshair::defaults();
        c.set_float("GameplayOption.Crosshair.Center.Radius", 99.0);
        assert_eq!(c.values["GameplayOption.Crosshair.Center.Radius"], "20.0");
        c.set_float("GameplayOption.Sights.ReticleSize", 0.9600001);
        assert_eq!(c.values["GameplayOption.Sights.ReticleSize"], "0.96");
    }

    #[test]
    fn short_code_is_short_and_round_trips() {
        let mut c = Crosshair::defaults();
        c.name = "Mine".into();
        assert!(c.to_short_code().len() < 60, "{}", c.to_short_code());
        c.set_float("GameplayOption.Crosshair.Center.Radius", 7.0);
        c.set_flag("GameplayOption.Crosshair.StaticReticle.Enabled", true);
        let short = c.to_short_code();
        assert!(short.len() < c.to_share_code().len() / 4, "{short}");
        assert_eq!(Crosshair::from_share_code(&short).unwrap(), c);
    }

    #[test]
    fn share_code_round_trips_and_validates() {
        let mut c = Crosshair::defaults();
        c.name = "Mine".into();
        c.set_choice("GameplayOption.Crosshair.Center.Shape", "Circle");
        c.set_rgb("GameplayOption.Crosshair.Center.Color", [10, 20, 30]);
        let back = Crosshair::from_share_code(&c.to_share_code()).unwrap();
        assert_eq!(
            back.choice("GameplayOption.Crosshair.Center.Shape"),
            "Circle"
        );
        assert_eq!(
            back.rgb("GameplayOption.Crosshair.Center.Color"),
            [10, 20, 30]
        );

        let bad = r#"{"name":"x","values":{"GameplayOption.Crosshair.Center.Radius":"500"}}"#;
        assert!(Crosshair::from_share_code(bad).is_err());
        let unknown = r#"{"name":"x","values":{"GameplayOption.Audio.MasterVolume":"1.0"}}"#;
        assert!(
            Crosshair::from_share_code(unknown).is_err(),
            "non-crosshair keys must be refused"
        );
        assert!(Crosshair::from_share_code("hello").is_err());
    }

    #[test]
    fn oversized_codes_are_refused_before_parsing() {
        let big = format!(
            r#"{{"name":"x","values":{{"a":"{}"}}}}"#,
            "9".repeat(MAX_CODE_BYTES)
        );
        assert!(
            Crosshair::from_share_code(&big)
                .unwrap_err()
                .contains("too long")
        );
    }

    #[test]
    fn unknown_top_level_fields_are_dropped_not_kept() {
        let code = r#"{"name":"x","values":{},"evil":"<script>alert(1)</script>"}"#;
        let c = Crosshair::from_share_code(code).unwrap();
        assert!(!c.to_share_code().contains("evil"));
    }

    #[test]
    fn nan_and_infinite_numbers_are_refused() {
        for bad in ["NaN", "inf", "-inf", "1e999"] {
            let code = format!(
                r#"{{"name":"x","values":{{"GameplayOption.Crosshair.Center.Radius":"{bad}"}}}}"#
            );
            assert!(
                Crosshair::from_share_code(&code).is_err(),
                "{bad} must be refused"
            );
        }
    }

    #[test]
    fn diff_from_defaults_is_minimal_and_merges_back() {
        let mut c = Crosshair::defaults();
        assert!(c.non_default_values().is_empty());
        c.set_float("GameplayOption.Crosshair.Center.Radius", 9.0);
        c.set_flag("GameplayOption.Sights.Enabled", true);
        let diff = c.non_default_values();
        assert!(diff.len() <= 2, "{diff:?}");
        assert_eq!(diff["GameplayOption.Crosshair.Center.Radius"], "9.0");
        let defaults: BTreeMap<String, String> =
            serde_json::from_str(&Crosshair::defaults_json()).unwrap();
        assert_eq!(defaults.len(), Crosshair::defaults().values.len());
    }
}
