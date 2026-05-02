//! Tempora-redirects table.
//!
//! Loads the upstream `Tabulae/Tempora/Generale.txt` (vendored as
//! `data/tempora_redirects.txt`) and exposes a rubric-aware stem
//! redirect lookup. The upstream Perl uses this table in
//! `Directorium::load_tempora` to swap a bare Tempora stem for the
//! rubric's preferred form.
//!
//! Format: `Tempora/<from>=Tempora/<to>;;<rubric-tokens>`. Each line
//! applies only to rubrics whose `transfer` token (per
//! `Tabulae/data.txt`) appears in the `<rubric-tokens>` column. We
//! drop comment lines and the explicit `XXXXX` "removed" entries.
//!
//! Rubric → transfer-token mapping (from upstream `data.txt`):
//!
//! | Rust `Rubric`        | upstream version label     | token  |
//! |----------------------|----------------------------|--------|
//! | Tridentine1570       | "Tridentine - 1570"        | 1570   |
//! | Tridentine1910       | "Tridentine - 1910"        | 1906   |
//! | DivinoAfflatu1911    | "Divino Afflatu - 1939"    | DA     |
//! | Reduced1955          | "Reduced - 1955"           | 1960   |
//! | Rubrics1960          | "Rubrics 1960 - 1960"      | 1960   |
//! | Monastic             | "Monastic Tridentinum…"    | M1617  |
//!
//! `DA` does not appear in the redirect file, so DA-1939 simply
//! falls through (no redirects fire).

use crate::divinum_officium::core::Rubric;
use std::collections::HashMap;
use std::sync::OnceLock;

static TABLE_TXT: &str = include_str!("../../data/tempora_redirects.txt");

/// Each `from` stem maps to a list of `(to_stem, rubric_tokens)`
/// entries; we keep all rubric variants (e.g. `Pasc3-0` can redirect
/// to `Pasc3-0t` under 1888/1906 *and* to `Pasc3-0r` under 1570/1960).
#[derive(Debug, Clone)]
struct RedirectRule {
    to: String,
    rubrics: Vec<String>,
}

static PARSED: OnceLock<HashMap<String, Vec<RedirectRule>>> = OnceLock::new();

fn parsed() -> &'static HashMap<String, Vec<RedirectRule>> {
    PARSED.get_or_init(|| parse(TABLE_TXT))
}

/// Map our `Rubric` enum to the upstream transfer token used in the
/// redirect file's third column.
pub(crate) fn rubric_token(rubric: Rubric) -> &'static str {
    match rubric {
        Rubric::Tridentine1570 => "1570",
        Rubric::Tridentine1910 => "1906",
        Rubric::DivinoAfflatu1911 => "DA",
        Rubric::Reduced1955 => "1960",
        Rubric::Rubrics1960 => "1960",
        Rubric::Monastic => "M1617",
    }
}

/// Resolve a bare Tempora stem under the active rubric. Returns the
/// redirect target if any rule applies, else `None` (caller keeps
/// the bare stem).
pub fn redirect(stem: &str, rubric: Rubric) -> Option<&'static str> {
    let token = rubric_token(rubric);
    let rules = parsed().get(stem)?;
    for rule in rules {
        if rule.rubrics.iter().any(|t| t == token) {
            return Some(rule.to.as_str());
        }
    }
    None
}

/// Backward-compat shim: equivalent to `redirect(stem,
/// Rubric::Tridentine1570)`.
pub fn redirect_1570(stem: &str) -> Option<&'static str> {
    redirect(stem, Rubric::Tridentine1570)
}

fn parse(text: &str) -> HashMap<String, Vec<RedirectRule>> {
    let mut out: HashMap<String, Vec<RedirectRule>> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (lhs, rhs_with_rubrics) = match line.split_once('=') {
            Some(p) => p,
            None => continue,
        };
        let (rhs, rubrics_col) = match rhs_with_rubrics.split_once(";;") {
            Some(p) => p,
            None => (rhs_with_rubrics, ""),
        };
        // Skip non-Tempora entries (e.g. C05-18=Votive/Coronatio for
        // commune-mass redirects — different layer).
        let from_full = lhs.trim();
        let from = match from_full.strip_prefix("Tempora/") {
            Some(s) => s,
            None => continue,
        };
        let to_full = rhs.trim();
        if to_full == "XXXXX" || to_full.is_empty() {
            continue;
        }
        let to = to_full.strip_prefix("Tempora/").unwrap_or(to_full);
        let rubrics: Vec<String> = rubrics_col
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        out.entry(from.to_string())
            .or_default()
            .push(RedirectRule {
                to: to.to_string(),
                rubrics,
            });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adv1_0_redirects_under_1570_only() {
        // Adv1-0 → Adv1-0o is a 1570-only redirect.
        assert_eq!(redirect("Adv1-0", Rubric::Tridentine1570), Some("Adv1-0o"));
        assert_eq!(redirect("Adv1-0", Rubric::Tridentine1910), None);
        assert_eq!(redirect("Adv1-0", Rubric::DivinoAfflatu1911), None);
    }

    #[test]
    fn quad3_3_redirects_only_under_1570() {
        // The bug we're fixing: Quad3-3 → Quad3-3t is 1570-only.
        // Under T1910 (token `1906`) no redirect should fire.
        assert_eq!(redirect("Quad3-3", Rubric::Tridentine1570), Some("Quad3-3t"));
        assert_eq!(redirect("Quad3-3", Rubric::Tridentine1910), None);
        assert_eq!(redirect("Quad3-3", Rubric::DivinoAfflatu1911), None);
        assert_eq!(redirect("Quad3-3", Rubric::Reduced1955), None);
        assert_eq!(redirect("Quad3-3", Rubric::Rubrics1960), None);
    }

    #[test]
    fn pent02_5_t1910_picks_o_variant() {
        // Sacred Heart (Friday after Corpus Christi octave): under
        // T1910 (1906) Pent02-5 redirects to Pent02-5o; under 1570 it
        // redirects to Pent02-5Feria. Verify both.
        assert_eq!(
            redirect("Pent02-5", Rubric::Tridentine1910),
            Some("Pent02-5o")
        );
        assert_eq!(
            redirect("Pent02-5", Rubric::Tridentine1570),
            Some("Pent02-5Feria")
        );
        // DA-1939 has no entry → no redirect.
        assert_eq!(redirect("Pent02-5", Rubric::DivinoAfflatu1911), None);
    }

    #[test]
    fn pasc3_0_branches_by_rubric() {
        // 1570 + 1960 + Newcal → Pasc3-0r (Patrocinii fallback).
        // 1888 + 1906 → Pasc3-0t (T1910 specific).
        assert_eq!(redirect("Pasc3-0", Rubric::Tridentine1570), Some("Pasc3-0r"));
        assert_eq!(redirect("Pasc3-0", Rubric::Tridentine1910), Some("Pasc3-0t"));
        assert_eq!(redirect("Pasc3-0", Rubric::Reduced1955), Some("Pasc3-0r"));
        assert_eq!(redirect("Pasc3-0", Rubric::Rubrics1960), Some("Pasc3-0r"));
        assert_eq!(redirect("Pasc3-0", Rubric::DivinoAfflatu1911), None);
    }

    #[test]
    fn epi1_0_redirects_to_a_variant() {
        // Epi1-0 → Epi1-0a applies under 1570/1888/1906 (pre-Holy-Family).
        assert_eq!(redirect("Epi1-0", Rubric::Tridentine1570), Some("Epi1-0a"));
        assert_eq!(redirect("Epi1-0", Rubric::Tridentine1910), Some("Epi1-0a"));
        // DA and later: Holy Family was instituted, so no redirect.
        assert_eq!(redirect("Epi1-0", Rubric::DivinoAfflatu1911), None);
    }

    #[test]
    fn nat02_xxxxx_is_skipped() {
        // Nat02=XXXXX means "no Tempora file under 1570" — the
        // sanctoral kalendar entry handles that day.
        assert_eq!(redirect("Nat02", Rubric::Tridentine1570), None);
    }

    #[test]
    fn unknown_stem_returns_none() {
        assert_eq!(redirect("Pent99-3", Rubric::Tridentine1570), None);
    }

    #[test]
    fn pent01_0_has_no_redirect() {
        // Trinity Sunday already existed in 1570 — bare stem applies.
        assert_eq!(redirect("Pent01-0", Rubric::Tridentine1570), None);
    }
}
