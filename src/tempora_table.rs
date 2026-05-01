//! Tempora-redirects table for Tridentine 1570.
//!
//! Loads the upstream `Tabulae/Tempora/Generale.txt` (vendored as
//! `data/tempora_redirects_1570.txt`, filtered to 1570-applicable
//! lines) and exposes a stem-redirect lookup. The upstream Perl uses
//! this table in `getsetup()` / `setupstring()` to swap a bare
//! Tempora stem for the rubric's preferred form — under 1570 the bare
//! `Tempora/Adv1-0` is post-Pius-V's adjusted Sunday-Mass, while the
//! original Tridentine Mass lives at `Tempora/Adv1-0o`.
//!
//! Format: `Tempora/<from>=Tempora/<to>;;<rubric-words>`. We keep
//! only entries where `1570` appears in the rubric-words column.
//! Comment lines and the explicit `XXXXX` "removed" entries are
//! treated as "no redirect" (we keep the bare stem and let the
//! caller decide).

use std::collections::HashMap;
use std::sync::OnceLock;

static TABLE_TXT: &str = include_str!("../../data/tempora_redirects_1570.txt");
static PARSED: OnceLock<HashMap<String, String>> = OnceLock::new();

fn parsed() -> &'static HashMap<String, String> {
    PARSED.get_or_init(|| parse(TABLE_TXT))
}

/// Resolve a bare Tempora stem (no `Tempora/` prefix) to its 1570
/// redirect, if any. Returns `None` when the table doesn't carry an
/// entry for this stem (or when the entry is `XXXXX` = "removed in
/// 1570" — same as no entry for our purposes).
pub fn redirect_1570(stem: &str) -> Option<&'static str> {
    parsed().get(stem).map(String::as_str)
}

fn parse(text: &str) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Format: `Tempora/<from>=Tempora/<to>;;<rubrics>`
        let (lhs, rhs_with_rubrics) = match line.split_once('=') {
            Some(p) => p,
            None => continue,
        };
        let (rhs, rubrics) = match rhs_with_rubrics.split_once(";;") {
            Some(p) => p,
            None => (rhs_with_rubrics, ""),
        };
        if !rubrics.split_whitespace().any(|w| w == "1570") {
            continue;
        }
        let from = lhs.trim().strip_prefix("Tempora/").unwrap_or(lhs.trim());
        let to = rhs.trim();
        if to == "XXXXX" || to.is_empty() {
            continue;
        }
        let to_stem = to.strip_prefix("Tempora/").unwrap_or(to);
        out.insert(from.to_string(), to_stem.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adv1_0_redirects_to_o_variant() {
        assert_eq!(redirect_1570("Adv1-0"), Some("Adv1-0o"));
    }

    #[test]
    fn epi1_0_redirects_to_a_variant() {
        assert_eq!(redirect_1570("Epi1-0"), Some("Epi1-0a"));
    }

    #[test]
    fn quad5_5_redirects_to_feriat() {
        assert_eq!(redirect_1570("Quad5-5"), Some("Quad5-5Feriat"));
    }

    #[test]
    fn pasc3_0_redirects_to_r_variant() {
        assert_eq!(redirect_1570("Pasc3-0"), Some("Pasc3-0r"));
    }

    #[test]
    fn pent01_0_has_no_redirect() {
        // Trinity Sunday already existed in 1570 — bare stem applies.
        assert_eq!(redirect_1570("Pent01-0"), None);
    }

    #[test]
    fn nat02_xxxxx_is_skipped() {
        // Nat02=XXXXX means "no Tempora file under 1570" — the
        // sanctoral kalendar entry handles that day.
        assert_eq!(redirect_1570("Nat02"), None);
    }

    #[test]
    fn unknown_stem_returns_none() {
        assert_eq!(redirect_1570("Pent99-3"), None);
    }
}
