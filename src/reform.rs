//! Reform-layer model.
//!
//! Each historical reform from Pius V (1570) through John XXIII
//! (1960) is a `ReformLayer` value bundling:
//!
//!   * a kalendar diff (saints suppressed / demoted / added / moved)
//!   * rubric overrides (precedence rule changes)
//!   * corpus overrides (e.g. Pius XII's new Easter Vigil propers)
//!
//! `reform_chain(rubric)` returns the active stack for that rubric
//! set: a Tridentine-1570-only render walks just `[PIUS_V_1570]`; a
//! 1962 Missale render walks
//! `[PIUS_V_1570, TRIDENT_1910, PIUS_X_1911, PIUS_XII_1955, JOHN_XXIII_1960]`,
//! applying each layer in order.
//!
//! Phase 1 ships the *shape* — the per-layer constants exist but
//! their diff/override structs are empty. Phases 7–10 fill them, one
//! reform per phase, with regression coverage at each step.
//!
//! See `DIVINUM_OFFICIUM_PORT_PLAN.md` "Reform stack" for the
//! Pope-by-Pope rationale.

use crate::divinum_officium::core::Rubric;

// Empty in Phase 1; structure emerges in Phases 7-10. Named-field
// empty structs (rather than unit) so adding fields later doesn't
// force a syntactic change at every initialisation site.

#[derive(Debug, Default)]
pub struct KalendarDiff {}

#[derive(Debug, Default)]
pub struct RubricOverrides {}

#[derive(Debug, Default)]
pub struct CorpusOverrides {}

#[derive(Debug)]
pub struct ReformLayer {
    pub name: &'static str,
    pub year: i32,
    pub kalendar_diff: KalendarDiff,
    pub rubric_overrides: RubricOverrides,
    pub corpus_overrides: CorpusOverrides,
}

// ─── Per-layer constants ─────────────────────────────────────────────
//
// One static per reform. References to these are cheap (`&'static
// ReformLayer`); the `reform_chain` slices below are also `'static`.

pub static PIUS_V_1570: ReformLayer = ReformLayer {
    name: "Pius V — 1570 (Quo primum)",
    year: 1570,
    kalendar_diff: KalendarDiff {},
    rubric_overrides: RubricOverrides {},
    corpus_overrides: CorpusOverrides {},
};

pub static TRIDENT_1910: ReformLayer = ReformLayer {
    name: "Tridentine kalendar as of 1910",
    year: 1910,
    kalendar_diff: KalendarDiff {},
    rubric_overrides: RubricOverrides {},
    corpus_overrides: CorpusOverrides {},
};

pub static PIUS_X_1911: ReformLayer = ReformLayer {
    name: "Pius X — Divino Afflatu 1911",
    year: 1911,
    kalendar_diff: KalendarDiff {},
    rubric_overrides: RubricOverrides {},
    corpus_overrides: CorpusOverrides {},
};

pub static PIUS_XII_1955: ReformLayer = ReformLayer {
    name: "Pius XII — Cum nostra hac aetate 1955",
    year: 1955,
    kalendar_diff: KalendarDiff {},
    rubric_overrides: RubricOverrides {},
    corpus_overrides: CorpusOverrides {},
};

pub static JOHN_XXIII_1960: ReformLayer = ReformLayer {
    name: "John XXIII — Rubricæ generales 1960",
    year: 1960,
    kalendar_diff: KalendarDiff {},
    rubric_overrides: RubricOverrides {},
    corpus_overrides: CorpusOverrides {},
};

pub static MONASTIC: ReformLayer = ReformLayer {
    name: "Monastic",
    year: 0, // separate chain — not part of the Roman reform stack
    kalendar_diff: KalendarDiff {},
    rubric_overrides: RubricOverrides {},
    corpus_overrides: CorpusOverrides {},
};

/// Returns the active reform stack for `rubric`. The stack is
/// chronological; consumers walk it in order, applying each layer's
/// effect on top of the prior cumulative state.
pub fn reform_chain(rubric: Rubric) -> &'static [&'static ReformLayer] {
    static CHAIN_1570: &[&ReformLayer] = &[&PIUS_V_1570];
    static CHAIN_1910: &[&ReformLayer] = &[&PIUS_V_1570, &TRIDENT_1910];
    static CHAIN_DA:   &[&ReformLayer] = &[&PIUS_V_1570, &TRIDENT_1910, &PIUS_X_1911];
    static CHAIN_1955: &[&ReformLayer] = &[
        &PIUS_V_1570, &TRIDENT_1910, &PIUS_X_1911, &PIUS_XII_1955,
    ];
    static CHAIN_1960: &[&ReformLayer] = &[
        &PIUS_V_1570, &TRIDENT_1910, &PIUS_X_1911, &PIUS_XII_1955, &JOHN_XXIII_1960,
    ];
    static CHAIN_MON:  &[&ReformLayer] = &[&MONASTIC];

    match rubric {
        Rubric::Tridentine1570    => CHAIN_1570,
        Rubric::Tridentine1910    => CHAIN_1910,
        Rubric::DivinoAfflatu1911 => CHAIN_DA,
        Rubric::Reduced1955       => CHAIN_1955,
        Rubric::Rubrics1960       => CHAIN_1960,
        Rubric::Monastic          => CHAIN_MON,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pius_v_in_every_roman_chain() {
        for &r in Rubric::ALL_ROMAN {
            let chain = reform_chain(r);
            assert!(
                chain.iter().any(|l| std::ptr::eq(*l, &PIUS_V_1570)),
                "{r:?} chain missing PIUS_V_1570"
            );
        }
    }

    #[test]
    fn chain_lengths_match_reform_count() {
        assert_eq!(reform_chain(Rubric::Tridentine1570).len(),    1);
        assert_eq!(reform_chain(Rubric::Tridentine1910).len(),    2);
        assert_eq!(reform_chain(Rubric::DivinoAfflatu1911).len(), 3);
        assert_eq!(reform_chain(Rubric::Reduced1955).len(),       4);
        assert_eq!(reform_chain(Rubric::Rubrics1960).len(),       5);
        assert_eq!(reform_chain(Rubric::Monastic).len(),          1);
    }

    #[test]
    fn chains_are_chronological() {
        // Within any chain, layer years must be non-decreasing.
        // Monastic year is 0 (sentinel for "off-axis"), excluded.
        for &r in Rubric::ALL_ROMAN {
            let chain = reform_chain(r);
            let mut prev = 0;
            for layer in chain {
                assert!(
                    layer.year >= prev,
                    "{r:?} chain not chronological: {} after year {}",
                    layer.year, prev
                );
                prev = layer.year;
            }
        }
    }

    #[test]
    fn monastic_separate_from_roman_chain() {
        let mon = reform_chain(Rubric::Monastic);
        assert_eq!(mon.len(), 1);
        assert_eq!(mon[0].name, "Monastic");
        // Roman base layer must NOT appear in the Monastic chain.
        assert!(!mon.iter().any(|l| std::ptr::eq(*l, &PIUS_V_1570)));
    }
}
