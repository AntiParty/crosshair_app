//! The companion website: open its pages and import a crosshair from a link.
//!
//! The app only ever talks to one address (`SITE`), only over https, never follows redirects, and reads at most
//! 16 KB. Whatever comes back is checked by the same validator as a pasted code, so a hostile or broken answer
//! can't put anything into the editor that a pasted code couldn't.

use std::time::Duration;

use finals_crosshair_core::model::Crosshair;

pub const SITE: &str = "https://crosshairs.finalsrs.com";
const MAX_BODY: u64 = 16 * 1024;

pub fn browse_url() -> String {
    SITE.to_string()
}

pub fn publish_url() -> String {
    format!("{SITE}/publish")
}

/// The id from a crosshair link (`https://crosshairs.finalsrs.com/c/abcd2345`) or a bare 8-character id.
/// Anything else, including a pasted code, is not a link.
pub fn parse_target(input: &str) -> Option<String> {
    let s = input.trim().to_ascii_lowercase();
    let host = SITE.trim_start_matches("https://");
    let rest = s
        .strip_prefix(&format!("{SITE}/c/"))
        .or_else(|| s.strip_prefix(&format!("{host}/c/")));
    let (id, tail) = match rest {
        Some(r) if r.len() >= 8 && r.is_char_boundary(8) => r.split_at(8),
        Some(_) => return None,
        None if s.len() == 8 => (s.as_str(), ""),
        None => return None,
    };
    let id_ok = id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    let tail_ok = tail.is_empty() || tail.starts_with(['/', '?', '#']);
    (id_ok && tail_ok).then(|| id.to_string())
}

/// Downloads a published crosshair's code and validates it.
pub fn fetch(id: &str) -> Result<Crosshair, String> {
    fetch_from(SITE, id, true)
}

fn fetch_from(base: &str, id: &str, https_only: bool) -> Result<Crosshair, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .max_redirects(0)
        .https_only(https_only)
        .user_agent("finals-crosshair-app")
        .build()
        .into();
    let mut resp = agent
        .get(&format!("{base}/c/{id}/code"))
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(404) => {
                "No crosshair with that link. It may have been removed.".to_string()
            }
            ureq::Error::StatusCode(429) => "The site is busy. Try again in a minute.".to_string(),
            ureq::Error::StatusCode(c) => format!("The site answered with an error ({c})."),
            _ => "Couldn't reach the site. Check your connection.".to_string(),
        })?;
    if resp.status().as_u16() != 200 {
        // redirects are never followed, so a 3xx lands here as well
        return Err(format!(
            "The site answered with an error ({}).",
            resp.status().as_u16()
        ));
    }
    let text = resp
        .body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_string()
        .map_err(|_| "The site's answer was too large or unreadable.".to_string())?;
    Crosshair::from_share_code(&text)
}

/// Opens one of our own pages in the default browser. Only the fixed addresses above are ever passed in.
pub fn open(url: &str) {
    debug_assert!(url.starts_with(SITE));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(0x0800_0000) // no console window
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn links_and_ids_are_recognised() {
        let ok = |s: &str| parse_target(s);
        assert_eq!(ok("abcd2345").as_deref(), Some("abcd2345"));
        assert_eq!(ok("  ABCD2345 \n").as_deref(), Some("abcd2345"));
        assert_eq!(
            ok("https://crosshairs.finalsrs.com/c/abcd2345").as_deref(),
            Some("abcd2345")
        );
        assert_eq!(
            ok("https://crosshairs.finalsrs.com/c/abcd2345/").as_deref(),
            Some("abcd2345")
        );
        assert_eq!(
            ok("https://crosshairs.finalsrs.com/c/abcd2345?m=published").as_deref(),
            Some("abcd2345")
        );
        assert_eq!(
            ok("crosshairs.finalsrs.com/c/abcd2345").as_deref(),
            Some("abcd2345")
        );
    }

    #[test]
    fn other_addresses_and_codes_are_not_links() {
        for s in [
            "",
            "short",
            "abcd23456",
            "https://evil.example/c/abcd2345",
            "http://crosshairs.finalsrs.com/c/abcd2345",
            "https://crosshairs.finalsrs.com.evil.example/c/abcd2345",
            "https://crosshairs.finalsrs.com/c/abcd2345evil",
            "https://crosshairs.finalsrs.com/c/abc",
            "https://crosshairs.finalsrs.com/u/abcd2345",
            "abcd-345",
            r#"{"name":"x","values":{}}"#,
            "ééééééé",
        ] {
            assert_eq!(parse_target(s), None, "{s}");
        }
    }

    /// A one-shot local server that answers with the given raw response.
    fn serve(response: Vec<u8>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let _ = s.write_all(&response);
            }
        });
        format!("http://{addr}")
    }

    fn http(status: &str, extra: &str, body: &[u8]) -> Vec<u8> {
        let mut r = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",
            body.len()
        )
        .into_bytes();
        r.extend_from_slice(body);
        r
    }

    #[test]
    fn a_valid_code_is_fetched_and_validated() {
        let mut c = Crosshair::defaults();
        c.name = "From the site".into();
        let base = serve(http(
            "200 OK",
            "Content-Type: text/plain\r\n",
            c.to_share_code().as_bytes(),
        ));
        let got = fetch_from(&base, "abcd2345", false).unwrap();
        assert_eq!(got.name, "From the site");
    }

    #[test]
    fn bad_answers_are_refused() {
        let cases: Vec<(Vec<u8>, &str)> = vec![
            (http("404 Not Found", "", b"no"), "No crosshair"),
            (http("429 Too Many Requests", "", b"slow"), "busy"),
            (http("500 Oops", "", b"x"), "error (500)"),
            (http("200 OK", "", b"this is not a crosshair"), ""),
            (
                http(
                    "200 OK",
                    "",
                    br#"{"name":"x","values":{"GameplayOption.Crosshair.Center.Radius":"9999"}}"#,
                ),
                "",
            ),
            (http("200 OK", "", &vec![b'a'; 40 * 1024]), "too large"),
            (
                http("302 Found", "Location: http://127.0.0.1:1/x\r\n", b""),
                "error (302)",
            ),
        ];
        for (resp, needle) in cases {
            let base = serve(resp);
            let err = fetch_from(&base, "abcd2345", false).expect_err("must be refused");
            assert!(err.contains(needle), "{err:?} should mention {needle:?}");
        }
    }

    #[test]
    fn plain_http_is_refused_in_production_mode() {
        let base = serve(http("200 OK", "", b"x"));
        assert!(fetch_from(&base, "abcd2345", true).is_err());
    }
}
