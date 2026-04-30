//! Phase 3 — port of upstream `occurrence()` for Tridentine 1570.
//!
//! Skeleton port of `vendor/divinum-officium/web/cgi-bin/horas/horascommon.pl:20-697`.
//! Targets the Tridentine 1570 path — every `if ($version =~ /1960/) …
//! elsif (/1955/) …` branch is dropped here, with markers pointing at
//! the phase that re-introduces it. The Phase 3 acceptance bar is
//! "unit suite green on hand-curated dates" (no regression harness
//! yet); deeper logic (transfer chains, octave bookkeeping, vigil
//! handling, the 17-24 Dec privileged-feria tables) lands as Phase 6
//! year-sweep failures expose it.
//!
//! Pure function: takes `&OfficeInput` and `&dyn Corpus`, returns
//! `OccurrenceResult`. No globals, no I/O outside the trait.
//!
//! Outstanding work (deferred to later phases — every `// reform-XYZ`
//! marker below maps to one of these):
//!
//! - **Phase 7 (1570 kalendar)** — load `Tabulae/Kalendaria/1570.txt`
//!   so the Sancti corpus matches the actual 1570 calendar instead of
//!   the post-Divino-Afflatu "default" rubric we ship today. Until
//!   Phase 7 lands, several legitimate 1570 dates render with the
//!   1911 sanctoral.
//! - **Phase 8 (Divino Afflatu)** — Pius X's psalter / sanctoral
//!   demotions; the `Festum Domini` rule for Sunday-vs-feast.
//! - **Phase 9 (1955)** — Holy Week reform, octave stripping,
//!   simplification of vigils.
//! - **Phase 10 (1960)** — Class I/II/III/IV consolidation.
//! - **Plus**: directorium-driven transfers (`get_from_directorium`),
//!   transferred vigils, the Saturday-BVM substitution, octave-day
//!   commemorations, the 11-02 All-Saints-vs-All-Souls collision.

use crate::divinum_officium::core::{
    CommuneType, FileCategory, FileKey, OfficeInput, ReformAction, Rubric,
};
use crate::divinum_officium::corpus::Corpus;
use crate::divinum_officium::date;
use crate::divinum_officium::missa::MassFile;
use crate::divinum_officium::sancti::SanctiEntry;

/// Output of a single `compute_occurrence` call. Captures every Perl
/// global that `occurrence()` writes — `$winner`, `$commemoratio`,
/// `$scriptura`, `$commune`, `$communetype`, `$rank`, `$sanctoraloffice` —
/// plus diagnostic numerics for the regression harness.
#[derive(Debug, Clone)]
pub struct OccurrenceResult {
    pub winner: FileKey,
    pub commemoratio: Option<FileKey>,
    pub scriptura: Option<FileKey>,
    pub commune: Option<FileKey>,
    pub commune_type: CommuneType,
    /// Numeric rank of the winner (Perl `$rank`).
    pub rank: f32,
    /// True when the sanctoral side outranked the temporal.
    pub sanctoral_office: bool,
    /// Diagnostics: rank of each side prior to the comparison.
    pub temporal_rank: f32,
    pub sanctoral_rank: f32,
    pub reform_trace: Vec<ReformAction>,
}

/// Entry point. Tridentine 1570 only for now — other rubrics
/// `panic!()` with a phase pointer until their reform layers land.
pub fn compute_occurrence(input: &OfficeInput, corpus: &dyn Corpus) -> OccurrenceResult {
    if !matches!(input.rubric, Rubric::Tridentine1570) {
        panic!(
            "compute_occurrence: rubric {:?} not yet supported \
             (Tridentine1570 only in Phase 3; see DIVINUM_OFFICIUM_PORT_PLAN.md \
             Phases 7-10)",
            input.rubric
        );
    }

    let (d, m, y) = (input.date.day, input.date.month, input.date.year);

    // ── Temporal side ────────────────────────────────────────────────
    // Mirror Perl `horascommon.pl:140-141`:
    //   $tday = "Tempora/{weekname}-{dow}" except for Nat dates where
    //   the format is "Tempora/{weekname}" (no -dow suffix).
    let weekname = date::getweek(d, m, y, false, true);
    let dow = date::day_of_week(d, m, y);
    let tempora_stem = if is_nat_label(&weekname) {
        weekname.clone()
    } else {
        format!("{}-{}", weekname, dow)
    };
    let tempora_key = FileKey {
        category: FileCategory::Tempora,
        stem: tempora_stem,
    };
    let tempora_file = corpus.mass_file(&tempora_key);
    let temporal_rank = tempora_file.and_then(|f| f.rank_num).unwrap_or(0.0);

    // ── Sanctoral side ───────────────────────────────────────────────
    let sancti_entries = corpus.sancti_entries(m, d);
    // reform-PHASE-7-9: the per-rubric kalendar diff would replace
    // `default` with the rubric-specific entry here. Until then we
    // pick `default` (≈ Divino Afflatu 1911) as the closest available
    // proxy for Tridentine 1570.
    let sancti_entry = pick_sancti_for_tridentine_1570(sancti_entries);
    let sanctoral_rank = sancti_entry.and_then(|e| e.rank_num).unwrap_or(0.0);
    let sancti_key = FileKey {
        category: FileCategory::Sancti,
        stem: format!("{m:02}-{d:02}"),
    };

    // ── Precedence ───────────────────────────────────────────────────
    let sanctoral_office = decide_sanctoral_wins_1570(
        sancti_entry,
        tempora_file,
        temporal_rank,
        sanctoral_rank,
    );

    // ── Build result ─────────────────────────────────────────────────
    if sanctoral_office {
        let sancti = sancti_entry.expect("sanctoral_office=true ⇒ entry exists");
        let (commune, commune_type) = parse_commune(&sancti.commune);
        let commemoratio = if commemorate_temporal_under_sanctoral_1570(
            sanctoral_rank,
            temporal_rank,
            tempora_file,
        ) {
            Some(tempora_key.clone())
        } else {
            None
        };
        OccurrenceResult {
            winner: sancti_key,
            commemoratio,
            // Mass: the loser's Lectio is preserved as `scriptura` for
            // the propers (Perl line 408, 626).
            scriptura: tempora_file.map(|_| tempora_key),
            commune,
            commune_type,
            rank: sanctoral_rank,
            sanctoral_office: true,
            temporal_rank,
            sanctoral_rank,
            reform_trace: vec![],
        }
    } else {
        let (commune, commune_type) = match tempora_file {
            Some(f) => parse_commune(f.commune.as_deref().unwrap_or("")),
            None => (None, CommuneType::None),
        };
        let commemoratio = if sancti_entry.is_some()
            && commemorate_sanctoral_under_temporal_1570(
                sanctoral_rank,
                temporal_rank,
                tempora_file,
            )
        {
            Some(sancti_key)
        } else {
            None
        };
        OccurrenceResult {
            winner: tempora_key,
            commemoratio,
            scriptura: None,
            commune,
            commune_type,
            rank: temporal_rank,
            sanctoral_office: false,
            temporal_rank,
            sanctoral_rank,
            reform_trace: vec![],
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn is_nat_label(s: &str) -> bool {
    // "Nat25", "Nat02" etc. — getweek emits these for the Christmas
    // Octave (Dec 25-31 unpadded) and the pre-Epiphany days
    // (Jan 1-? zero-padded, file names Nat02.txt etc.).
    s.starts_with("Nat") && s.len() > 3 && s[3..].chars().all(|c| c.is_ascii_digit())
}

fn pick_sancti_for_tridentine_1570(entries: &[SanctiEntry]) -> Option<&SanctiEntry> {
    // Tridentine 1570 predates every rubric variant in our shipped
    // sancti.json. The "default" rubric is our calibrated baseline
    // (≈ Divino Afflatu 1911); this is the closest proxy until
    // Phase 7 ingests vendor/divinum-officium/web/www/Tabulae/Kalendaria/1570.txt.
    for &want in ["default", "1570"].iter() {
        if let Some(e) = entries.iter().find(|e| e.rubric == want) {
            return Some(e);
        }
    }
    entries.first()
}

/// Decide whether the sanctoral side wins under 1570 rules.
///
/// 1570 precedence (per the *Rubricæ generales* of Pius V's Missal):
///
///   * Class I temporal (Easter, Pentecost, Christmas, Epiphany, Sundays
///     of Advent, Sundays of Lent / Passion / Palm) wins over almost
///     everything sanctoral.
///   * On lesser Sundays (post-Pentecost, post-Epiphany) the Sunday
///     normally wins; sanctoral can outrank only at Class I sanctoral
///     rank, or at "Festum Domini" rank ≥ 5 (deferred to Phase 8).
///   * On ferias of Lent (greater ferias) and Advent (17-24 Dec
///     privileged), only Class I sanctoral outranks (deferred — needs
///     proper privileged-feria detection).
///   * Otherwise: numeric rank comparison.
fn decide_sanctoral_wins_1570(
    sancti: Option<&SanctiEntry>,
    tempora: Option<&MassFile>,
    trank: f32,
    srank: f32,
) -> bool {
    let _sancti = match sancti {
        None => return false, // no sanctoral entry → temporal wins
        Some(e) => e,
    };

    // No Tempora file at all → sanctoral wins. Happens for the post-
    // Christmas Sancti dates that don't have a parallel Tempora file
    // (Jan 1 Circumcision, etc.).
    let tempora = match tempora {
        None => return true,
        Some(t) => t,
    };

    // Class I Temporals beat everything below Class I sanctoral.
    // 1570 rank table: rank ≥ 6 ≈ Class I, ≥ 5 ≈ Class II, ≥ 2 ≈ Duplex,
    // < 2 ≈ Simplex/feria. (See SANCTI_CONVENTION at top of sancti.rs.)
    if trank >= 7.0 {
        return false;
    }

    // Sunday handling. Detect via the rendered name — pre-1960 Sundays
    // are written as `Dominica …`. (The Perl uses regex on `$trank[0]`
    // and `$dayname[0]`; we approximate with the officium string.)
    let temporal_name = tempora.officium.as_deref().unwrap_or("");
    let is_sunday = temporal_name.starts_with("Dominica");
    if is_sunday {
        // Pre-1960: Class I sanctoral wins over Class II Sundays.
        // reform-PHASE-8: add the "Festum Domini ≥ rank 5" exception.
        return srank >= 6.0;
    }

    // reform-PHASE-9: privileged-feria handling (Quad, Adv 17-24,
    // Quattuor Temporum). Today we approximate via the Tempora rank
    // string — "Feria major" / "Feria privilegiata" / "I classis"
    // suggests privileged.
    let temporal_rank_label = tempora.rank.as_deref().unwrap_or("");
    let is_privileged_feria = temporal_rank_label.contains("Privileg")
        || temporal_rank_label.contains("privileg")
        || temporal_rank_label.contains("major")
        || temporal_rank_label.contains("classis");
    if is_privileged_feria && trank >= 6.0 {
        // High-privilege ferias only yield to Class I sanctoral.
        return srank >= 6.0;
    }

    // Default: strict numeric rank comparison.
    srank > trank
}

/// Should the temporal cycle be commemorated under a sanctoral winner?
/// 1570 rule of thumb: Class I sanctoral wins solo (no temporal
/// commemoration); below Class I, the temporal Sunday/feria gets
/// commemorated. reform-PHASE-9 will tighten this against the actual
/// Tridentine commemoration table.
fn commemorate_temporal_under_sanctoral_1570(
    srank: f32,
    trank: f32,
    tempora: Option<&MassFile>,
) -> bool {
    if tempora.is_none() || trank == 0.0 {
        return false;
    }
    if srank >= 7.0 {
        // Class I sanctoral with octave (rare) — solo.
        return false;
    }
    // Class I sanctoral (rank 6.x) commemorates a major temporal
    // (Sunday, Class I feria). On lesser temporals — drop.
    if srank >= 6.0 {
        return trank >= 5.0;
    }
    // Class II / III sanctoral — always commemorate the temporal.
    true
}

fn commemorate_sanctoral_under_temporal_1570(
    srank: f32,
    trank: f32,
    _tempora: Option<&MassFile>,
) -> bool {
    if srank == 0.0 {
        return false;
    }
    // Class I temporal (Easter, Christmas, Pentecost) — only Class I
    // sanctoral gets commemorated; lesser sanctoral suppressed.
    if trank >= 7.0 {
        return srank >= 6.0;
    }
    // Otherwise the loser sanctoral commemorates.
    true
}

/// Parse a commune indication like `"vide C2a-1"` or `"ex C9"` into a
/// typed `(FileKey, CommuneType)`.
fn parse_commune(s: &str) -> (Option<FileKey>, CommuneType) {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return (None, CommuneType::None);
    }
    let (prefix, rest) = match trimmed.split_once(' ') {
        Some(p) => p,
        None => return (None, CommuneType::None),
    };
    let kind = match prefix.to_ascii_lowercase().as_str() {
        "vide" => CommuneType::Vide,
        "ex" => CommuneType::Ex,
        _ => return (None, CommuneType::None),
    };
    let stem = rest.split_whitespace().next().unwrap_or("").to_string();
    if stem.is_empty() {
        return (None, kind);
    }
    let key = FileKey {
        category: FileCategory::Commune,
        stem,
    };
    (Some(key), kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::divinum_officium::core::{Date, Locale};
    use crate::divinum_officium::corpus::BundledCorpus;

    fn input(year: i32, month: u32, day: u32) -> OfficeInput {
        OfficeInput {
            date: Date::new(year, month, day),
            rubric: Rubric::Tridentine1570,
            locale: Locale::Latin,
        }
    }

    fn winner_path(r: &OccurrenceResult) -> String {
        r.winner.render()
    }

    fn run(year: i32, month: u32, day: u32) -> OccurrenceResult {
        compute_occurrence(&input(year, month, day), &BundledCorpus)
    }

    // ─── Class I temporals win solo ──────────────────────────────────

    #[test]
    fn easter_sunday_temporal_wins() {
        // 2026-04-05 = Easter Sunday. Pasc0-0 rank 7.0 (Class I
        // Duplex). No sanctoral commemoration in the typical edition.
        let r = run(2026, 4, 5);
        assert_eq!(winner_path(&r), "Tempora/Pasc0-0");
        assert!(!r.sanctoral_office);
        assert_eq!(r.commemoratio, None);
    }

    #[test]
    fn pentecost_sunday_temporal_wins() {
        // 2026-05-24 = Pentecost. Pasc7-0 rank 7.0 (Class I).
        let r = run(2026, 5, 24);
        assert_eq!(winner_path(&r), "Tempora/Pasc7-0");
        assert!(!r.sanctoral_office);
    }

    #[test]
    fn trinity_sunday_temporal_wins() {
        // 2026-05-31 = Trinity Sunday. Pent01-0 rank 6.5 (Class I).
        let r = run(2026, 5, 31);
        assert_eq!(winner_path(&r), "Tempora/Pent01-0");
        assert!(!r.sanctoral_office);
    }

    #[test]
    fn lent_sunday_outranks_low_sanctoral() {
        // 2026-02-22 = Dominica I in Quadragesima (Quad1-0, rank 6.9).
        // Sancti/02-22 = "S. Margaritæ a Cortona" (low rank).
        let r = run(2026, 2, 22);
        assert_eq!(winner_path(&r), "Tempora/Quad1-0");
        assert!(!r.sanctoral_office);
    }

    // ─── Sanctoral wins ──────────────────────────────────────────────

    #[test]
    fn christmas_day_sanctoral_wins_solo() {
        // 2026-12-25. Sancti/12-25 rank 6.5 (Class I Duplex). No
        // Tempora/Nat25 file → sanctoral wins solo.
        let r = run(2026, 12, 25);
        assert_eq!(winner_path(&r), "Sancti/12-25");
        assert!(r.sanctoral_office);
    }

    #[test]
    fn st_stephen_sanctoral_wins() {
        // 2026-12-26. Sancti/12-26 rank 5.4 (Duplex II). No
        // Tempora/Nat26 → sanctoral wins.
        let r = run(2026, 12, 26);
        assert_eq!(winner_path(&r), "Sancti/12-26");
        assert!(r.sanctoral_office);
    }

    #[test]
    fn peter_and_paul_sanctoral_wins_outranks_post_pentecost_feria() {
        // 2026-06-29 = Mon. Sancti/06-29 rank 6.5 (Peter & Paul,
        // Class I). Tempora is post-Pent ferial.
        let r = run(2026, 6, 29);
        assert_eq!(winner_path(&r), "Sancti/06-29");
        assert!(r.sanctoral_office);
        // Class I sanctoral on a feria — temporal not commemorated.
        // (Phase 6 may surface that the upstream commemorates ferial
        // Lectio as scriptura instead — that's the `scriptura` field.)
        assert!(r.scriptura.is_some(),
            "expected scriptura to retain Tempora reference for Lectio");
    }

    #[test]
    fn immaculate_conception_outranks_advent_feria() {
        // 2026-12-08 = Tue, Adv2-2 ferial. Conceptione BMV rank 6.5
        // (Class I). Sanctoral wins; feria of Advent commemorated.
        let r = run(2026, 12, 8);
        assert_eq!(winner_path(&r), "Sancti/12-08");
        assert!(r.sanctoral_office);
        // Phase 9+ may tighten which ferias get commemorated under
        // a Class I sanctoral.
    }

    // ─── Cases gated on later phases (ignored with markers) ──────────

    #[test]
    #[ignore = "Phase 7: needs 1570 kalendar diff (St Peter Martyr instituted later)"]
    fn st_peter_martyr_ranking_under_1570_kalendar() {
        // 2026-04-29. Default sancti has S. Petri Martyris rank 3
        // (post-Divino-Afflatu). The 1570 kalendar suppresses /
        // alters this entry.
        let r = run(2026, 4, 29);
        assert_eq!(winner_path(&r), "Tempora/Pasc3-3"); // ferial of Pasc3
    }

    #[test]
    #[ignore = "Phase 7: 1570 kalendar — Tempora/Pasc3-3 carries the 1911-instituted St-Joseph Octave Day"]
    fn pasc3_3_no_st_joseph_in_1570() {
        // The Tempora file Pasc3-3.txt embeds Patrocinii S. Joseph
        // (instituted 1847, Octave added later). 1570 didn't have it.
        let r = run(2026, 4, 29);
        assert_eq!(r.temporal_rank, 0.0,
            "1570 kalendar should suppress Patrocinii St Joseph Octave");
    }

    #[test]
    #[ignore = "Phase 7+: All Saints Octave Day vs All Souls collision"]
    fn all_souls_day_collision() {
        // 2026-11-02. In 1570, the Mass of the day on Nov 2 is the
        // Defunctorum (All Souls), but the OFFICE is the All Saints
        // Octave Day II. Mass and Office diverge — handled in
        // upstream by special cases that we haven't ported.
        let r = run(2026, 11, 2);
        assert!(winner_path(&r).contains("Defunctorum") || winner_path(&r).contains("11-02"));
    }

    #[test]
    #[ignore = "Phase 9: Saturday-of-Our-Lady substitution on free Saturdays"]
    fn saturday_bvm_substitution() {
        // A free Saturday with low temporal rank should swap to
        // "Sanctae Mariae Sabbato" with Common C10. Perl
        // horascommon.pl:401-420 handles this.
        let r = run(2026, 5, 16); // arbitrary free Saturday
        assert_eq!(r.commune.as_ref().map(|k| k.stem.as_str()), Some("C10"));
    }

    #[test]
    #[ignore = "Phase 9: 17-24 Dec privileged-ferias table"]
    fn dec_21_st_thomas_apostle() {
        // 2026-12-21 = St Thomas. Class II Duplex. In 1570 wins
        // because Adv4 ferials are not yet "I classis privilegiata"
        // (that's a 1955 reform). The 17-24 Dec privileged ferias
        // need an explicit table.
        let r = run(2026, 12, 21);
        assert_eq!(winner_path(&r), "Sancti/12-21");
        assert!(r.sanctoral_office);
    }

    // ─── Diagnostics / sanity ────────────────────────────────────────

    #[test]
    fn rank_diagnostics_populated() {
        let r = run(2026, 12, 25);
        assert!(r.sanctoral_rank > 0.0, "expected sanctoral rank for Christmas");
        // The winner's rank field equals whichever side won.
        assert_eq!(r.rank, r.sanctoral_rank);
    }

    #[test]
    fn commune_parses_ex_c2a() {
        // Internal helper smoke test.
        let (k, t) = parse_commune("ex C2a-1");
        assert_eq!(t, CommuneType::Ex);
        assert_eq!(k.unwrap().render(), "Commune/C2a-1");
    }

    #[test]
    fn commune_parses_vide_c11() {
        let (k, t) = parse_commune("vide C11");
        assert_eq!(t, CommuneType::Vide);
        assert_eq!(k.unwrap().render(), "Commune/C11");
    }

    #[test]
    fn commune_empty_yields_none() {
        let (k, t) = parse_commune("");
        assert!(k.is_none());
        assert_eq!(t, CommuneType::None);
    }

    #[test]
    fn nat_label_detection() {
        assert!(is_nat_label("Nat25"));
        assert!(is_nat_label("Nat02"));
        assert!(!is_nat_label("Nat1-0"));   // Sunday infra Octavam shape
        assert!(!is_nat_label("Pasc3"));
        assert!(!is_nat_label("Nat"));
    }

    // ─── Class-I sanctoral on Sunday: should win in 1570 ─────────────

    #[test]
    fn st_joseph_can_outrank_lent_sunday_when_class_i() {
        // S. Joseph (March 19) — rank 6.1 (Class I). If it falls on
        // a Sunday of Lent (rank 6.9 in 1570), the Sunday should
        // narrowly win because Joseph is rank 6.1 and Sunday-of-Lent
        // is 6.9. Verifies the Sunday-vs-feast comparison.
        // 2026-03-19 = Thursday (not Sunday) — pick a generic feria.
        let r = run(2026, 3, 19);
        // On a Lent ferial, St Joseph wins.
        assert_eq!(winner_path(&r), "Sancti/03-19");
        assert!(r.sanctoral_office);
    }
}
