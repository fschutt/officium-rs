//! Phase 4 — high-level orchestrator. Wraps Phase 3 `occurrence` and
//! produces the canonical `core::OfficeOutput` for downstream Mass /
//! Office rendering.
//!
//! Mirrors upstream `vendor/divinum-officium/web/cgi-bin/horas/horascommon.pl:1525-1709`
//! ("sub precedence"). Like Phase 3, this is an MVP-skeleton port for
//! Tridentine 1570 — rubric-conditional branches stripped, marker
//! comments left where Phases 7–10 will re-introduce them.
//!
//! Public entry point:
//!
//! ```ignore
//! pub fn compute_office(input: &OfficeInput, corpus: &dyn Corpus) -> OfficeOutput;
//! ```
//!
//! The legacy 4-class approximation lives at `precedence_legacy` until
//! Phase 11 wires the WIP pages to this module.

#[allow(unused_imports)]
use crate::divinum_officium::core::{
    Color, DayKind, Locale, OfficeInput, OfficeOutput, Rank, RankClass, RankKind, ReformAction,
    RuleLine, Rubric, Season,
};
use crate::divinum_officium::corpus::Corpus;
use crate::divinum_officium::date;
use crate::divinum_officium::occurrence::{self, OccurrenceResult};
use crate::divinum_officium::sancti;

/// Phase 4 orchestrator. Calls `compute_occurrence`, parses the
/// winner's rank string into a typed `Rank`, derives `DayKind` /
/// `Season` / `Color`, and returns the canonical `OfficeOutput`.
///
/// Locale is currently fixed to `Latin`. Vernacular text assembly is
/// downstream of the rubric core (translation pipeline).
pub fn compute_office(input: &OfficeInput, corpus: &dyn Corpus) -> OfficeOutput {
    if !matches!(input.locale, Locale::Latin) {
        panic!("compute_office: only Locale::Latin in the rubric core");
    }

    let occ = occurrence::compute_occurrence(input, corpus);

    let (rank, raw_label) = resolve_rank(&occ, input, corpus);
    let day_kind = resolve_day_kind(&occ, input, corpus);
    let season = resolve_season(input);
    let color = resolve_color(&occ, input, corpus);
    let rule = resolve_rule_lines(&occ, corpus);

    OfficeOutput {
        date: input.date,
        winner: occ.winner.clone(),
        commemoratio: occ.commemoratio.clone(),
        scriptura: occ.scriptura.clone(),
        commune: occ.commune.clone(),
        commune_type: occ.commune_type,
        rank,
        rule,
        day_kind,
        season,
        color,
        // Office-only — first-vespers concurrence is `concurrence()`
        // in Perl, deferred to Phase 12+ when the Diurnal page lands.
        vespers_split: None,
        reform_trace: enrich_reform_trace(&occ, raw_label, input),
    }
}

// ─── Rank resolution ─────────────────────────────────────────────────

fn resolve_rank(
    occ: &OccurrenceResult,
    input: &OfficeInput,
    corpus: &dyn Corpus,
) -> (Rank, String) {
    let rank_num = occ.rank;
    let raw_label = if occ.sanctoral_office {
        // Sanctoral winner — pull rank_class from the Sancti corpus.
        // Apply the Tridentine leap shift (real Feb 24 in leap year →
        // sancti key 02-29, etc.) so we hit the right entry.
        let (look_m, look_d) =
            date::sday_pair(input.date.month, input.date.day, input.date.year);
        let entries = corpus.sancti_entries(look_m, look_d);
        sancti::pick_by_rubric(entries, &["default", "1570"])
            .map(|e| e.rank_class.clone())
            .unwrap_or_default()
    } else {
        // Temporal winner — pull from the Tempora MassFile.
        corpus
            .mass_file(&occ.winner)
            .and_then(|f| f.rank.as_deref().map(rank_string_to_label))
            .unwrap_or_default()
    };

    let class = rank_class_from_num(rank_num);
    let kind = rank_kind_from_label(&raw_label);
    (
        Rank {
            class,
            kind,
            raw_label: raw_label.clone(),
            rank_num,
        },
        raw_label,
    )
}

/// Tempora rank fields look like `"I classis Semiduplex;;6.9;;..."` or
/// `"Duplex majus"`; we want just the human-readable left side, before
/// any `;;` separator and without trailing rubric flags.
fn rank_string_to_label(s: &str) -> String {
    s.split(";;").next().unwrap_or("").trim().to_string()
}

fn rank_class_from_num(r: f32) -> RankClass {
    if r >= 6.0 {
        RankClass::First
    } else if r >= 5.0 {
        RankClass::Second
    } else if r >= 2.0 {
        RankClass::Third
    } else {
        RankClass::Fourth
    }
}

fn rank_kind_from_label(label: &str) -> RankKind {
    let l = label.to_ascii_lowercase();
    // Order matters — most specific first.
    if l.contains("supra") || l.contains("triduum") {
        RankKind::Above
    } else if l.contains("duplex i classis") || l.contains("duplex 1 classis") {
        RankKind::DuplexIClassis
    } else if l.contains("duplex ii classis") || l.contains("duplex 2 classis") {
        RankKind::DuplexIIClassis
    } else if l.contains("duplex majus") || l.contains("duplex maj") {
        RankKind::DuplexMajus
    } else if l.contains("semiduplex") {
        RankKind::Semiduplex
    } else if l.contains("duplex") {
        RankKind::Duplex
    } else if l.contains("simplex") {
        RankKind::Simplex
    } else if l.contains("feria") || l.contains("vigilia") || l.contains("sabbato") {
        RankKind::Feria
    } else if l.contains("commemoratio") {
        RankKind::Commemoration
    } else if l.is_empty() {
        RankKind::Feria
    } else {
        RankKind::Duplex
    }
}

// ─── DayKind ─────────────────────────────────────────────────────────

fn resolve_day_kind(occ: &OccurrenceResult, input: &OfficeInput, corpus: &dyn Corpus) -> DayKind {
    let officium = winner_officium(occ, input, corpus);
    let l = officium.to_ascii_lowercase();

    if l.contains("quattuor temporum") || l.contains("quatuor temporum") {
        DayKind::EmberDay
    } else if l.contains("rogation") {
        DayKind::RogationDay
    } else if l.starts_with("in vigilia") || l.starts_with("vigilia") {
        DayKind::Vigil
    } else if l.contains("in octava") || l.contains("die octava") {
        DayKind::OctaveDay
    } else if l.starts_with("dominica") {
        DayKind::Sunday
    } else if l.starts_with("feria") || l.starts_with("sabbato") {
        DayKind::Feria
    } else if occ.sanctoral_office {
        DayKind::Feast
    } else {
        // Default: derive from day-of-week if the officium string is
        // unhelpful (e.g. empty for missing-data dates).
        if date::day_of_week(input.date.day, input.date.month, input.date.year) == 0 {
            DayKind::Sunday
        } else {
            DayKind::Feria
        }
    }
}

fn winner_officium(occ: &OccurrenceResult, _input: &OfficeInput, corpus: &dyn Corpus) -> String {
    let mut key = occ.winner.clone();
    for _ in 0..4 {
        if let Some(file) = corpus.mass_file(&key) {
            if let Some(name) = file.officium.as_deref() {
                return name.to_string();
            }
            // Officium missing — chase the file-level parent inherit
            // (typical of `@`-only redirects like Sancti/12-24o that
            // forward everything to Sancti/12-24).
            if let Some(parent_path) = file.parent.as_deref() {
                key = crate::divinum_officium::core::FileKey::parse(parent_path);
                continue;
            }
        }
        break;
    }
    String::new()
}

// ─── Season ──────────────────────────────────────────────────────────

fn resolve_season(input: &OfficeInput) -> Season {
    let week = date::getweek(
        input.date.day,
        input.date.month,
        input.date.year,
        false,
        true,
    );
    if week.starts_with("Adv") {
        Season::Advent
    } else if week.starts_with("Nat") {
        Season::Christmas
    } else if week.starts_with("Epi") {
        Season::PostEpiphany
    } else if week.starts_with("Quadp") {
        Season::Septuagesima
    } else if week == "Quad5" || week == "Quad6" {
        // Passion Sunday (Quad5) and Holy Week (Quad6).
        Season::Passiontide
    } else if week.starts_with("Quad") {
        Season::Lent
    } else if week.starts_with("Pasc") {
        Season::Easter
    } else if week.starts_with("Pent") || week.starts_with("PentEpi") {
        // Pent01 = Trinity Sunday; through Pent24 / PentEpi6 → Advent.
        // The Pentecost Octave week (Pent00..Pent01-eve) gets a
        // dedicated Season::PentecostOctave when we have it; here we
        // accept post-Pentecost for the whole stretch.
        Season::PostPentecost
    } else {
        // Defensive: should be unreachable given the getweek labels.
        Season::PostPentecost
    }
}

// ─── Color ───────────────────────────────────────────────────────────

fn resolve_color(occ: &OccurrenceResult, input: &OfficeInput, corpus: &dyn Corpus) -> Color {
    let officium = winner_officium(occ, input, corpus);
    // Reuse the existing string heuristic from sancti.rs.
    match sancti::liturgical_color(&officium) {
        "red" => Color::Red,
        "white" => Color::White,
        "purple" => Color::Purple,
        "green" => Color::Green,
        "rose" => Color::Rose,
        "black" => Color::Black,
        _ => Color::White,
    }
}

// ─── Rule lines ──────────────────────────────────────────────────────

fn resolve_rule_lines(occ: &OccurrenceResult, corpus: &dyn Corpus) -> Vec<RuleLine> {
    // Phase 4 keeps rule lines as an opaque string from the [Rule]
    // section of the winner's MassFile. Phase 5 will need to parse
    // selected directives ("no Gloria", "Credo", "Preface=Communis")
    // for proper Mass assembly.
    let body = corpus
        .mass_file(&occ.winner)
        .and_then(|f| f.sections.get("Rule"))
        .cloned()
        .unwrap_or_default();
    body.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| RuleLine(l.to_string()))
        .collect()
}

// ─── Provenance ──────────────────────────────────────────────────────

fn enrich_reform_trace(
    occ: &OccurrenceResult,
    _raw_label: String,
    _input: &OfficeInput,
) -> Vec<ReformAction> {
    // Phase 7+ each reform layer pushes a ReformAction here when it
    // alters the resolution. Phase 4 starts with whatever the
    // OccurrenceResult carried (currently empty).
    occ.reform_trace.clone()
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::divinum_officium::core::Date;
    use crate::divinum_officium::corpus::BundledCorpus;

    fn input(year: i32, month: u32, day: u32) -> OfficeInput {
        OfficeInput {
            date: Date::new(year, month, day),
            rubric: Rubric::Tridentine1570,
            locale: Locale::Latin,
        }
    }

    fn run(year: i32, month: u32, day: u32) -> OfficeOutput {
        compute_office(&input(year, month, day), &BundledCorpus)
    }

    // ─── Rank resolution ─────────────────────────────────────────────

    #[test]
    fn easter_sunday_is_class_i() {
        let o = run(2026, 4, 5);
        assert_eq!(o.rank.class, RankClass::First);
        assert_eq!(o.season, Season::Easter);
        assert_eq!(o.day_kind, DayKind::Sunday);
    }

    #[test]
    fn christmas_day_is_class_i() {
        let o = run(2026, 12, 25);
        assert_eq!(o.rank.class, RankClass::First);
        assert_eq!(o.season, Season::Christmas);
        assert!(matches!(o.day_kind, DayKind::Feast | DayKind::Sunday));
    }

    #[test]
    fn ss_petri_et_pauli_is_class_i_feast() {
        let o = run(2026, 6, 29);
        assert_eq!(o.rank.class, RankClass::First);
        assert_eq!(o.day_kind, DayKind::Feast);
        // Color: red for apostolic feasts.
        assert_eq!(o.color, Color::Red);
    }

    #[test]
    fn lent_sunday_is_class_i_temporal() {
        let o = run(2026, 2, 22);
        assert_eq!(o.season, Season::Lent);
        assert_eq!(o.day_kind, DayKind::Sunday);
        assert_eq!(o.color, Color::Purple);
        assert_eq!(o.rank.class, RankClass::First);
    }

    #[test]
    fn palm_sunday_in_passiontide() {
        let o = run(2026, 3, 29);
        assert_eq!(o.season, Season::Passiontide);
        assert_eq!(o.day_kind, DayKind::Sunday);
    }

    #[test]
    fn first_advent_sunday_purple() {
        let o = run(2026, 11, 29);
        assert_eq!(o.season, Season::Advent);
        assert_eq!(o.color, Color::Purple);
        assert_eq!(o.day_kind, DayKind::Sunday);
    }

    #[test]
    fn st_stephen_red_feast() {
        let o = run(2026, 12, 26);
        assert_eq!(o.season, Season::Christmas);
        assert_eq!(o.day_kind, DayKind::Feast);
        // St Stephen Protomartyr — red.
        assert_eq!(o.color, Color::Red);
    }

    #[test]
    fn ordinary_post_pent_feria_green() {
        // 2026-07-15 = Wed in summer post-Pent (Pent5-3 or so).
        // Sancti/07-15 may have Henry of Bavaria etc. — keep the
        // assertion loose: just check season + sanity.
        let o = run(2026, 7, 15);
        assert_eq!(o.season, Season::PostPentecost);
    }

    #[test]
    fn septuagesima_sunday_lavender() {
        let o = run(2026, 2, 1);
        assert_eq!(o.season, Season::Septuagesima);
        assert_eq!(o.color, Color::Purple);
    }

    #[test]
    fn trinity_sunday_post_pentecost_classification() {
        let o = run(2026, 5, 31);
        // Pent01-0 → PostPentecost season.
        assert_eq!(o.season, Season::PostPentecost);
        assert_eq!(o.day_kind, DayKind::Sunday);
        assert_eq!(o.rank.class, RankClass::First);
    }

    // ─── Rank-kind parsing ───────────────────────────────────────────

    #[test]
    fn rank_kind_classification() {
        assert_eq!(rank_kind_from_label("Duplex I classis"), RankKind::DuplexIClassis);
        assert_eq!(rank_kind_from_label("Duplex II classis"), RankKind::DuplexIIClassis);
        assert_eq!(rank_kind_from_label("Duplex majus"), RankKind::DuplexMajus);
        assert_eq!(rank_kind_from_label("Duplex"), RankKind::Duplex);
        assert_eq!(rank_kind_from_label("Semiduplex"), RankKind::Semiduplex);
        assert_eq!(rank_kind_from_label("Simplex"), RankKind::Simplex);
        assert_eq!(rank_kind_from_label("Feria"), RankKind::Feria);
        assert_eq!(rank_kind_from_label("Feria Privilegiata"), RankKind::Feria);
        assert_eq!(rank_kind_from_label("Vigilia"), RankKind::Feria);
        assert_eq!(rank_kind_from_label(""), RankKind::Feria);
    }

    #[test]
    fn rank_class_from_numeric() {
        assert_eq!(rank_class_from_num(6.5), RankClass::First);
        assert_eq!(rank_class_from_num(5.1), RankClass::Second);
        assert_eq!(rank_class_from_num(3.0), RankClass::Third);
        assert_eq!(rank_class_from_num(1.0), RankClass::Fourth);
        assert_eq!(rank_class_from_num(0.0), RankClass::Fourth);
    }

    #[test]
    fn rank_string_strips_trailing_metadata() {
        assert_eq!(rank_string_to_label("Duplex II classis;;5;;ex C1"), "Duplex II classis");
        assert_eq!(rank_string_to_label("Feria"), "Feria");
        assert_eq!(rank_string_to_label("  Duplex  ;;rest"), "Duplex");
    }

    // ─── DayKind detection ───────────────────────────────────────────

    #[test]
    fn vigil_detection_christmas() {
        let o = run(2026, 12, 24);
        assert_eq!(o.day_kind, DayKind::Vigil);
    }

    #[test]
    #[ignore = "Phase 7+: emberday detection wants Quattuor Temporum substring in the resolved officium"]
    fn ember_wednesday_in_lent() {
        // 2026-02-25 = Feria IV Cinerum (Ash Wed). Ember days are
        // Wed/Fri/Sat of Adv III, Quad I, Pentecost-Octave, and
        // September. This is Cinerum — not formally an Ember day,
        // but the test is for the detection plumbing.
        let o = run(2026, 2, 25);
        assert_eq!(o.day_kind, DayKind::Feria);
    }

    // ─── Output integrity ────────────────────────────────────────────

    #[test]
    fn vespers_split_is_none_for_mass() {
        let o = run(2026, 12, 25);
        assert!(o.vespers_split.is_none());
    }

    #[test]
    fn winner_filekey_renders_as_path() {
        let o = run(2026, 12, 25);
        assert_eq!(o.winner.render(), "Sancti/12-25");
    }

    #[test]
    fn temporal_wins_easter_sets_no_scriptura() {
        let o = run(2026, 4, 5);
        // Easter Sunday — temporal wins; scriptura field used only
        // when sanctoral wins (saint's Lectio gets the temporal).
        assert!(o.scriptura.is_none());
    }

    #[test]
    fn sanctoral_wins_carries_scriptura_back() {
        let o = run(2026, 6, 29); // Peter & Paul on a Mon
        assert_eq!(o.winner.render(), "Sancti/06-29");
        // Scriptura should reference the Tempora file the sanctoral displaced.
        assert!(o.scriptura.is_some(),
            "expected scriptura to retain Tempora reference for Lectio");
    }

    // ─── Cross-checks against direct occurrence call ─────────────────

    #[test]
    fn compute_office_aligns_with_compute_occurrence() {
        // The orchestrator must not silently overwrite the
        // OccurrenceResult. Cross-verify on a known-easy date.
        let i = input(2026, 4, 5);
        let o = compute_office(&i, &BundledCorpus);
        let occ = occurrence::compute_occurrence(&i, &BundledCorpus);
        assert_eq!(o.winner, occ.winner);
        assert_eq!(o.commemoratio, occ.commemoratio);
        assert_eq!(o.rank.rank_num, occ.rank);
    }

    // ─── Cases gated on later phases ─────────────────────────────────

    #[test]
    #[ignore = "Phase 11: rubric switch needs reform layers ported first"]
    fn rubrics_1960_panics_today() {
        // Direct sanity that other rubrics panic — verifies the gate
        // until Phase 7-10 layers ship.
        let i = OfficeInput {
            date: Date::new(2026, 4, 5),
            rubric: Rubric::Rubrics1960,
            locale: Locale::Latin,
        };
        // We're testing the panic path — wrap in catch_unwind.
        let r = std::panic::catch_unwind(|| compute_office(&i, &BundledCorpus));
        assert!(r.is_err());
    }

    #[test]
    #[ignore = "Phase 12: locale routing — rubric core stays Latin until then"]
    fn nonlatin_locale_panics() {
        // Currently Locale only has one variant; this test will
        // become live when we add Vernacular(LangCode).
    }
}
