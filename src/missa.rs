//! Mass-corpus lookup. Loads `data/missa_latin.json` (built once from
//! the upstream Divinum Officium repo by `data/build_missa_json.py`)
//! and exposes a tree-walking accessor.
//!
//! The shipped JSON is keyed by `<dir>/<stem>` mirroring upstream
//! filenames (`Sancti/04-29`, `Tempora/Pasc3-0`, `Commune/C2a-1`, …).
//! Mass files reference each other via `@Commune/Cxx-y` markers inside
//! section bodies; `resolve_section()` chases those one hop.
//!
//! The on-disk format is postcard-encoded (built by `build.rs` from
//! the source `data/missa_latin.json`); the runtime decoder is
//! `postcard::from_bytes`, which is `no_std`-friendly. The struct
//! definition lives in [`crate::data_types::MassFile`] so that
//! `build.rs` and the lib agree on shape.

use std::collections::HashMap;
use std::sync::OnceLock;

pub use crate::data_types::MassFile;

static MISSA_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/missa_latin.postcard"));
static PARSED: OnceLock<HashMap<String, MassFile>> = OnceLock::new();

fn parsed() -> &'static HashMap<String, MassFile> {
    PARSED.get_or_init(|| postcard::from_bytes(MISSA_BIN).unwrap_or_default())
}

/// Look up a Mass file by key (e.g. `"Sancti/04-29"`,
/// `"Tempora/Pasc3-0"`, `"Commune/C2a-1"`).
pub fn lookup(key: &str) -> Option<&'static MassFile> {
    parsed().get(key)
}

/// Iterator over every loaded `(key, MassFile)` pair. Used by the
/// regression harness's reverse-lookup ("which file does this Perl
/// body actually come from?") — see `regression::infer_perl_source`.
pub fn iter() -> impl Iterator<Item = (&'static String, &'static MassFile)> {
    parsed().iter()
}

/// Resolve a section, chasing one `@Commune/<key>` reference if the
/// body is exactly such a marker. Multi-step reference chains and the
/// `@Commune/C2:Lectio7 in 4 loco` substitution form are intentionally
/// left for a later iteration (returns the raw marker string instead).
///
/// Returns `(body, source_key)` so the renderer can show "(from
/// Commune of one Martyr)" alongside derived sections.
pub fn resolve_section(file: &MassFile, section: &str) -> Option<(String, String)> {
    let body = file.sections.get(section)?.trim().to_string();
    if let Some(rest) = body.strip_prefix('@') {
        // `@Commune/C2a-1` (whole-section reference).
        let candidate = rest.split_whitespace().next().unwrap_or("");
        // Only follow simple references — reject the colon-suffixed
        // form (`@Commune/C2:Lectio7 in 4 loco`) for now.
        if !candidate.contains(':') && !candidate.contains(' ') {
            if let Some(other) = lookup(candidate) {
                if let Some(other_body) = other.sections.get(section) {
                    return Some((other_body.trim().to_string(), candidate.to_string()));
                }
            }
        }
    }
    Some((body, String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_st_peter_martyr_mass() {
        let f = lookup("Sancti/04-29").expect("Sancti/04-29 missing");
        assert_eq!(f.officium.as_deref(), Some("S. Petri Martyris"));
        assert_eq!(f.rank.as_deref(), Some("Duplex"));
        assert!(f.sections.contains_key("Oratio"));
    }

    #[test]
    fn resolve_commune_reference() {
        // [Lectio] body for Sancti/04-29 is `@Commune/C2a-1`.
        let f = lookup("Sancti/04-29").unwrap();
        let (body, src) = resolve_section(f, "Lectio").unwrap();
        assert_eq!(src, "Commune/C2a-1");
        assert!(!body.starts_with('@'));
        assert!(body.len() > 50, "expected a real Lectio body, got {body:?}");
    }

    #[test]
    fn tempora_sunday_pasc3() {
        let f = lookup("Tempora/Pasc3-0").expect("Tempora/Pasc3-0 missing");
        assert!(f.officium.as_deref().unwrap_or("").contains("III Post Pascha"));
        assert!(f.sections.contains_key("Introitus"));
        assert!(f.sections.contains_key("Evangelium"));
    }
}
