//! Latin Mass-Ordinary prayer corpus.
//!
//! Wraps `vendor/divinum-officium/web/www/missa/Latin/Ordo/Prayers.txt`
//! (vendored as `data/prayers_latin.txt`). The file holds named
//! prayer fragments (`[Gloria]`, `[Per Dominum]`, `[Dominus]`,
//! `[Confiteor]`, …) that the upstream renderer interpolates into
//! proper bodies via `&Macro` and `$Macro` tokens.
//!
//! See Phase 6.5 in `DIVINUM_OFFICIUM_PORT_PLAN.md`. The Rust pipeline
//! ships proper bodies with the literal `&Macro` / `$Macro` tokens
//! intact (they're how the upstream encodes "insert Glória Patri here"
//! without duplicating the text in 600 mass files); the comparator
//! needs the expansion to match what Perl renders, so the expander
//! lives next to this loader (see `mass::expand_macros`).
//!
//! The chosen file is the *Latin* Prayers.txt — i.e. the one whose
//! bodies match what the rendered HTML emits when locale=Latin. If
//! Phase 11 ever extends this to vernacular comparisons we'd add
//! per-locale variants here.

use std::collections::BTreeMap;
use std::sync::OnceLock;

static PRAYERS_TXT: &str = include_str!("../../data/prayers_latin.txt");
static PARSED: OnceLock<BTreeMap<String, String>> = OnceLock::new();
static PARSED_CI: OnceLock<BTreeMap<String, String>> = OnceLock::new();

fn parsed() -> &'static BTreeMap<String, String> {
    PARSED.get_or_init(|| parse(PRAYERS_TXT))
}

/// Lower-cased index for case-insensitive lookup. Some upstream Mass
/// files invoke macros with non-canonical casing (`&pater_noster` vs
/// `[Pater noster]`); the Perl `prayer()` regex chain folds case at
/// the substitution callsite, so we mirror that here.
fn parsed_ci() -> &'static BTreeMap<String, String> {
    PARSED_CI.get_or_init(|| {
        parsed()
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.clone()))
            .collect()
    })
}

/// Look up a prayer body by its `[Header]` name (case-sensitive).
pub fn lookup(name: &str) -> Option<&'static str> {
    parsed().get(name).map(String::as_str)
}

/// Case-insensitive lookup. `&pater_noster` ⇒ `[Pater noster]`,
/// `$per dominum` ⇒ `[Per Dominum]`, etc.
pub fn lookup_ci(name: &str) -> Option<&'static str> {
    parsed_ci().get(&name.to_lowercase()).map(String::as_str)
}

/// All known prayer-section names. Useful for diagnostics + tests.
pub fn names() -> Vec<&'static str> {
    parsed().keys().map(String::as_str).collect()
}

/// Parse the Prayers.txt format: a sequence of `[Header]` blocks, each
/// followed by lines of body until the next `[Header]` or EOF. Body
/// preserves blank lines internally but is trimmed of leading/trailing
/// blank lines. First-occurrence wins (the upstream sometimes carries
/// rubric-conditional duplicates which we ignore at this layer — Phase
/// 7+ kalendar diffs will introduce per-rubric overrides if needed).
fn parse(text: &str) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut buf: Vec<String> = Vec::new();
    for raw in text.lines() {
        let trimmed = raw.trim_end();
        if let Some(name) = parse_header(trimmed) {
            // Flush previous block.
            if let Some(prev) = current.take() {
                let body = trim_blank_edges(&buf);
                out.entry(prev).or_insert(body);
                buf.clear();
            }
            current = Some(name);
            continue;
        }
        if current.is_some() {
            buf.push(trimmed.to_string());
        }
    }
    // Flush final block.
    if let Some(prev) = current.take() {
        let body = trim_blank_edges(&buf);
        out.entry(prev).or_insert(body);
    }
    out
}

/// `[Header Name]` → `Some("Header Name")`. Anything else → `None`.
/// Tolerates trailing junk after `]` (e.g. comments, `(rubrica X)`)
/// the way build_missa_json.py does for Mass files.
fn parse_header(line: &str) -> Option<String> {
    let line = line.trim_start();
    if !line.starts_with('[') {
        return None;
    }
    let close = line.find(']')?;
    let name = line[1..close].trim();
    if name.is_empty() {
        return None;
    }
    // Reject `[1Nocturn]` Mass-section-style headers — Prayers.txt
    // headers start with an uppercase letter or accented char (`Pater`,
    // `Ave Maria`, `Confiteor`), never a digit. (Defensive — a stray
    // junk header in the file would otherwise silently swallow the
    // following body.)
    let first = name.chars().next()?;
    if first.is_ascii_digit() {
        return None;
    }
    Some(name.to_string())
}

fn trim_blank_edges(lines: &[String]) -> String {
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(lines.len());
    let end = lines.iter().rposition(|l| !l.trim().is_empty()).map(|i| i + 1).unwrap_or(0);
    if start >= end {
        return String::new();
    }
    lines[start..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_known_macros() {
        // The five macros that drive Phase 6.5 macro-expansion.
        assert!(lookup("Gloria").is_some());
        assert!(lookup("Per Dominum").is_some());
        assert!(lookup("Per eumdem").is_some());
        assert!(lookup("Per eundem").is_some());
        assert!(lookup("Qui tecum").is_some());
        assert!(lookup("Dominus vobiscum").is_some());
        assert!(lookup("Oremus").is_some());
        assert!(lookup("Pater noster").is_some());
        assert!(lookup("Credo").is_some());
        assert!(lookup("Gloria tibi").is_some());
    }

    #[test]
    fn gloria_body_contains_pater() {
        // [Gloria] = the Glória Patri (NOT Glória in excelsis — the
        // latter lives in OrdoA.txt and is invoked separately).
        let g = lookup("Gloria").expect("Gloria");
        assert!(g.contains("Glória Patri"), "Gloria: {g:?}");
        assert!(g.contains("Sicut erat"));
    }

    #[test]
    fn per_dominum_body_starts_with_per() {
        let p = lookup("Per Dominum").expect("Per Dominum");
        assert!(p.contains("Per Dóminum"), "Per Dominum: {p:?}");
        assert!(p.contains("vivit et regnat"));
    }

    #[test]
    fn per_eumdem_distinct_from_per_dominum() {
        let a = lookup("Per Dominum").unwrap();
        let b = lookup("Per eumdem").unwrap();
        assert_ne!(a, b);
        assert!(b.contains("eúndem"));
    }

    #[test]
    fn dominus_vobiscum_short_form() {
        let d = lookup("Dominus vobiscum").expect("Dominus vobiscum");
        assert!(d.contains("Dóminus vobíscum"));
        assert!(d.contains("Et cum spíritu tuo"));
    }

    #[test]
    fn oremus_minimal() {
        let o = lookup("Oremus").expect("Oremus");
        assert!(o.contains("Orémus"));
    }

    #[test]
    fn missing_macro_returns_none() {
        assert!(lookup("DefinitelyNotAPrayer").is_none());
        assert!(lookup("").is_none());
    }

    #[test]
    fn lookup_ci_handles_lowercase() {
        // `&pater_noster` → `[Pater noster]` via case-insensitive form.
        assert!(lookup_ci("pater noster").is_some());
        assert!(lookup_ci("PATER NOSTER").is_some());
        assert!(lookup_ci("PaTer NosTer").is_some());
        // Original case-sensitive lookup still works.
        assert!(lookup("Pater noster").is_some());
        // `lookup` rejects non-canonical case.
        assert!(lookup("pater noster").is_none());
        assert!(lookup("PATER NOSTER").is_none());
    }

    #[test]
    fn names_includes_core_set() {
        let n = names();
        for needle in [
            "Gloria",
            "Per Dominum",
            "Dominus vobiscum",
            "Oremus",
            "Pater noster",
            "Credo",
        ] {
            assert!(n.contains(&needle), "names() missing {needle:?}");
        }
    }

    #[test]
    fn parses_synthetic_block() {
        let txt = "[A]\nbody A line 1\nbody A line 2\n\n[B]\nbody B\n";
        let m = parse(txt);
        assert_eq!(m.get("A").map(String::as_str), Some("body A line 1\nbody A line 2"));
        assert_eq!(m.get("B").map(String::as_str), Some("body B"));
    }

    #[test]
    fn parse_first_occurrence_wins() {
        let txt = "[X]\nfirst body\n\n[X]\nsecond body\n";
        let m = parse(txt);
        assert_eq!(m.get("X").map(String::as_str), Some("first body"));
    }

    #[test]
    fn parse_rejects_digit_headers() {
        // `[1Nocturn]` style Office headers shouldn't be eaten by the
        // Mass prayer parser (defensive — the file we ship doesn't
        // contain any, but a future merge from upstream might).
        let txt = "[1Nocturn]\nbody\n";
        let m = parse(txt);
        assert!(m.is_empty(), "digit-prefixed header should not register");
    }
}
