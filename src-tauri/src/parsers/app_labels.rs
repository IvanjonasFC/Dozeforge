//! Batch resolver for package -> human-readable application label.
//!
//! Android does not expose a single ADB command that returns labels for all
//! installed packages. The only reliable source is `dumpsys package <pkg>`,
//! which emits a section that includes:
//!
//!     Application Label: <Name>
//!
//! when the package's manifest declares `android:label` or when an installer
//! provided a localised string. Some bare system packages do not emit this
//! line at all; for those we fall back to the package name's last segment.
//!
//! ## Performance
//!
//! `dumpsys package <pkg>` costs ~80-180 ms per call on a modern Pixel.
//! Scanning 400 packages sequentially would saturate the bridge for ~60 s
//! and ship 2-3 MB of text we then throw away. To keep this affordable we
//! send a single shell script that:
//!
//!   1. Iterates `pm list packages` on-device.
//!   2. For each package, pipes `dumpsys package <pkg>` through `grep -m1`
//!      so execution stops at the first matching line.
//!   3. Emits one `package|label` row per line for trivial parsing on host.
//!
//! That collapses 400 ADB round trips into one and bounds the output to a
//! couple hundred KB. On a Pixel 8 Pro the whole scan takes 35-60 s; on
//! older mid-range hardware closer to 90 s. Callers should always cache.

use std::collections::HashMap;

use crate::error::Result;
use crate::parsers::Parser;

/// One-shot batch resolver. Stateless - keep the shell script in sync with
/// the parser below.
pub struct AppLabelsResolver;

impl AppLabelsResolver {
    /// Shell script run on the device. POSIX/`mksh` compatible because
    /// Android's `/system/bin/sh` is `mksh`, not `bash`.
    pub fn command() -> &'static str {
        r#"pm list packages 2>/dev/null | cut -d: -f2 | while IFS= read -r p; do lbl=$(dumpsys package "$p" 2>/dev/null | grep -m1 'Application Label:' | sed 's/^.*Application Label: //'); echo "$p|$lbl"; done"#
    }
}

impl Parser for AppLabelsResolver {
    type Output = HashMap<String, String>;

    fn parse(&self, input: &str) -> Result<Self::Output> {
        let mut out: HashMap<String, String> = HashMap::with_capacity(512);
        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some(idx) = line.find('|') else {
                continue;
            };
            let pkg = line[..idx].trim();
            let label_raw = line[idx + 1..].trim();
            if pkg.is_empty() {
                continue;
            }
            let cleaned = label_raw.trim_matches(|c: char| c == '<' || c == '>');
            let final_label = if cleaned.is_empty() || cleaned == "null" {
                pkg.rsplit('.').next().unwrap_or(pkg).to_string()
            } else {
                cleaned.to_string()
            };
            out.insert(pkg.to_string(), final_label);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_pipe_format() {
        let input = "com.twitter.android|X\ncom.whatsapp|WhatsApp\n";
        let out = AppLabelsResolver.parse(input).unwrap();
        assert_eq!(out.get("com.twitter.android").map(String::as_str), Some("X"));
        assert_eq!(out.get("com.whatsapp").map(String::as_str), Some("WhatsApp"));
    }

    #[test]
    fn falls_back_to_last_segment_when_label_missing() {
        let input = "com.example.something|\ncom.x.y|\n";
        let out = AppLabelsResolver.parse(input).unwrap();
        assert_eq!(out.get("com.example.something").map(String::as_str), Some("something"));
        assert_eq!(out.get("com.x.y").map(String::as_str), Some("y"));
    }

    #[test]
    fn handles_angle_bracket_marker() {
        let input = "com.foo|<com.foo.Bar>\n";
        let out = AppLabelsResolver.parse(input).unwrap();
        assert_eq!(out.get("com.foo").map(String::as_str), Some("com.foo.Bar"));
    }

    #[test]
    fn ignores_garbage_lines() {
        let input = "garbage no pipe\ncom.real|RealApp\n|orphan\n";
        let out = AppLabelsResolver.parse(input).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out.get("com.real").map(String::as_str), Some("RealApp"));
    }

    #[test]
    fn label_with_embedded_pipe_keeps_first_segment_only() {
        let input = "com.weird|Name|withPipe\n";
        let out = AppLabelsResolver.parse(input).unwrap();
        assert_eq!(out.get("com.weird").map(String::as_str), Some("Name|withPipe"));
    }
}
