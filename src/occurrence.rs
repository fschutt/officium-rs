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

#[allow(unused_imports)]
use crate::core::{
    CommuneType, FileCategory, FileKey, OfficeInput, ReformAction, Rubric,
};
use crate::corpus::Corpus;
use crate::date;
use crate::kalendarium_1570;
use crate::missa::MassFile;
use crate::sancti::SanctiEntry;

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

/// Entry point. The 1570 baseline is the load-bearing rubric (99.7%
/// cross-validated against Perl). Other rubrics dispatch through the
/// same code path but consult their own `Layer` for kalendar
/// lookups — Tridentine 1910 reads PiusX1906, Divino Afflatu reads
/// PiusXI1939, etc. The rubric-RULE deltas (precedence, vigil
/// suppression, octave handling) are still 1570-shape until each
/// layer lands its rule overrides.
pub fn compute_occurrence(input: &OfficeInput, corpus: &dyn Corpus) -> OccurrenceResult {
    let layer = input.rubric.kalendar_layer();

    let (d, m, y) = (input.date.day, input.date.month, input.date.year);

    // ── Temporal side ────────────────────────────────────────────────
    // Mirror Perl `horascommon.pl:140-141`:
    //   $tday = "Tempora/{weekname}-{dow}" except for Nat dates where
    //   the format is "Tempora/{weekname}" (no -dow suffix).
    let weekname = date::getweek(d, m, y, false, true);
    let dow = date::day_of_week(d, m, y);
    let tempora_stem_default = if is_nat_label(&weekname) {
        weekname.clone()
    } else {
        format!("{}-{}", weekname, dow)
    };
    // Rubric-aware Tempora-stem redirect (mirrors upstream
    // `Directorium::load_tempora` reading `Tabulae/Tempora/Generale.txt`):
    //   * 1570 → `Epi1-0a`, `Quad3-3t`, `Pent02-5Feria`, etc.
    //   * 1888/1906 (T1910) → `Epi1-0a`, `Pent02-5o` (Sacred Heart),
    //     `Pasc3-0t` (3rd Sunday after Easter Tridentinum)
    //   * 1960 (R55/R60) → `Pasc3-0r` (Patrocinii fallback), Pent
    //     ferials → `…Feria`
    //   * DA-1939 (no token in the table) → no redirects fire.
    let tempora_stem = pick_tempora_variant(&tempora_stem_default, input.rubric, corpus);
    // September Embertide overlay: for Pent06+ ferias the upstream
    // `officestring` (SetupString.pl:720-779) overlays a monthday
    // file (e.g. `Tempora/093-6` for Sept Ember Saturday) onto the
    // bare Pent file. The Pent file (rank 1.0 Feria) gets eclipsed by
    // the monthday file's rank (e.g. 2.1 "Feria major"), which keeps
    // Saturday-BVM from firing and supplies the Sept Embertide
    // propers. We approximate the overlay by *replacing* the
    // tempora_stem with the monthday stem when both:
    //   1. The original stem is in `Pent06+` or `Epi*` (the Perl
    //      guard is `fname =~ /Pent|Epi/ && !/Pent0[1-5]/`).
    //   2. The monthday-derived file exists in the missa corpus.
    // For 1570 only `093-3 / 093-5 / 093-6` ship under missa, so the
    // overlay fires only on Sept Ember Wed/Fri/Sat.
    let tempora_stem = apply_monthday_overlay_1570(&tempora_stem, d, m, y, input.rubric, corpus);
    // Sunday-letter / Easter-coded Transfer table override
    // (`Tabulae/Transfer/<letter>.txt` and `<easter-code>.txt`,
    // filtered to `;;1570`). When the entry's main target starts
    // with `Tempora/`, it replaces the temporal winner — driving
    // the "Dominica anticipata" Saturday Mass (e.g. `01-31 →
    // Tempora/Epi4-0tt` in years where Septuagesima is so early
    // that Epi4 Sunday never lands on a Sunday). Bare-stem targets
    // (e.g. `11-28=11-29`) are handled later in the sancti
    // resolution path.
    let tempora_stem =
        apply_transfer_temporal_1570(&tempora_stem, y, m, d, input.rubric.transfer_rubric_tag());
    let tempora_key = FileKey {
        category: FileCategory::Tempora,
        stem: tempora_stem,
    };
    // Mass-context broken-redirect detection: when the missa-side
    // file is a bare path with no `@` prefix (sole known case:
    // `Tempora/Pasc1-0t.txt`), Perl's `SetupString::setupstring`
    // reads it as an empty stub (`__preamble` only, no Rank). The
    // saint of the day wins on Low Sunday because trank[2]=0 vs a
    // rank ≥ 1 Sancti. Office-context (default) still follows the
    // parent chain because `horas/Latin/Tempora/Pasc1-0t.txt` HAS
    // the proper `@`-prefix and inherits Pasc1-0's rank 7.
    // See `docs/UPSTREAM_WEIRDNESSES.md` #37.
    let mass_broken_redirect_active = input.is_mass_context
        && corpus
            .mass_file(&tempora_key)
            .map(|f| f.mass_broken_redirect)
            .unwrap_or(false);
    // For body-less redirect files (Tempora/Adv1-0o is just a single
    // `@Tempora/Adv1-0` parent-inherit line), follow the parent chain
    // to find the file that actually carries the rank/officium.
    let effective_tempora_key = if mass_broken_redirect_active {
        tempora_key.clone()
    } else {
        effective_tempora_key(&tempora_key, corpus)
    };
    let tempora_file = corpus.mass_file(&effective_tempora_key);
    // Rubric-aware rank pick. The corpus carries up to four
    // alternative `rank_num_*` slots populated from per-rubric `[Rank]
    // (rubrica …)` second-headers (Tempora/Pent02-5o elevates Sacred
    // Heart from 4.01 to 6.5 under T1910; Pent02-1 has 1570-only 2.9
    // override; etc.). Pick the slot that matches the active rubric,
    // then fall back to the bare default.
    let temporal_rank = if mass_broken_redirect_active {
        0.0
    } else {
        tempora_file
            .and_then(|f| match input.rubric {
                Rubric::Tridentine1570 => f.rank_num_1570.or(f.rank_num),
                Rubric::Tridentine1910 => f.rank_num_1906.or(f.rank_num),
                Rubric::DivinoAfflatu1911 => f.rank_num,
                Rubric::Reduced1955 => f.rank_num_1955.or(f.rank_num),
                Rubric::Rubrics1960 => f.rank_num_1960.or(f.rank_num_1955).or(f.rank_num),
                Rubric::Monastic => f.rank_num_1570.or(f.rank_num),
            })
            .map(|r| downgrade_post_1570_octave(r, tempora_file.unwrap(), input.rubric))
            .unwrap_or(0.0)
    };

    // ── Sanctoral side ───────────────────────────────────────────────
    let (sancti_key, sancti_entry_holder) =
        resolve_sancti_for_tridentine_1570(y, m, d, layer, input.rubric, corpus);
    let sancti_entry: Option<&SanctiEntry> = sancti_entry_holder.as_ref();
    let sanctoral_rank = sancti_entry.and_then(|e| e.rank_num).unwrap_or(0.0);

    // ── Precedence ───────────────────────────────────────────────────
    let sancti_mass_file = corpus.mass_file(&sancti_key);
    let sanctoral_office = decide_sanctoral_wins_1570(
        sancti_entry,
        tempora_file,
        temporal_rank,
        sanctoral_rank,
        input.rubric,
        sancti_mass_file,
    );

    // ── Anticipated Sunday-Within-Octave-of-Epiphany ────────────────
    // Under T1570 / T1910 the Octave of Epiphany was kept and Jan 13
    // (Octave Day) outranks the Sunday-Within-Octave Mass. When Jan 13
    // falls on a Sunday, Perl celebrates the Sunday-Within-Octave
    // ("Dominica infra Octavam Epiphaniæ ~ Semiduplex Dominica minor")
    // anticipated to Saturday Jan 12 — the Sunday Mass moves backward
    // rather than being suppressed entirely. Mirrors the directorium-
    // driven anticipation in `horascommon.pl`.
    //
    // DA / R55 / R60 handle the same case via explicit transfer-table
    // entries (`01-12=Tempora/Epi1-0;;DA`); this branch fills the gap
    // for the older rubrics where the table is silent.
    //
    // Closing this case took the multi-year regression sweep above
    // 99.86% → ≈99.90% (15 fail-years × 5 rubrics → 0).
    if matches!(input.rubric, Rubric::Tridentine1570 | Rubric::Tridentine1910)
        && m == 1
        && d == 12
        && dow == 6
    {
        let stem = pick_tempora_variant("Epi1-0", input.rubric, corpus);
        let key = FileKey {
            category: FileCategory::Tempora,
            stem,
        };
        return OccurrenceResult {
            winner: key.clone(),
            commemoratio: None,
            scriptura: Some(key),
            commune: None,
            commune_type: CommuneType::None,
            rank: 2.5, // Semiduplex Dominica minor
            sanctoral_office: false,
            temporal_rank: 2.5,
            sanctoral_rank,
            reform_trace: vec![],
        };
    }

    // ── Saturday-BVM rule (Tridentine 1570) ──────────────────────────
    // On free Saturdays (no major feast), the Mass is "Sanctæ Mariæ
    // Sabbato" using Commune/C10[a/b/c/Pasc] depending on the
    // liturgical season. Mirrors `horascommon.pl:401-420`.
    if let Some(saturday_bvm) = saturday_bvm_winner_1570(
        dow,
        &weekname,
        m,
        d,
        temporal_rank,
        sanctoral_rank,
    ) {
        return OccurrenceResult {
            winner: saturday_bvm.clone(),
            commemoratio: None,
            scriptura: tempora_file.map(|_| tempora_key.clone()),
            commune: Some(saturday_bvm),
            commune_type: CommuneType::Vide,
            rank: 1.3,
            sanctoral_office: true,
            temporal_rank,
            sanctoral_rank,
            reform_trace: vec![],
        };
    }

    // ── Build result ─────────────────────────────────────────────────
    if sanctoral_office {
        let sancti = sancti_entry.expect("sanctoral_office=true ⇒ entry exists");
        let (mut commune, mut commune_type) =
            parse_commune_in_context(&sancti.commune, &sancti_key.category);
        // Pius XII (1955) suppressed the Octave of the Epiphany —
        // Sancti/01-07 .. 01-12 are now bare class IV ferias. On those
        // weekdays the Mass is always the Sunday-after-Epiphany
        // ("In excelso throno"); on weekday 01-07 itself it stays the
        // Epiphany Mass ("Ecce advenit"), but the rest of that week
        // routes through `Tempora/Epi1-0a`. Mirrors
        // `horascommon.pl:1613-1622`:
        //
        //   if ($version =~ /19(?:55|6)/
        //     && $missa
        //     && $dayname[0] =~ /Epi1/i
        //     && $winner =~ /01\-([0-9]+)/
        //     && $1 < 13
        //     && $dayofweek != 0)
        //   { $communetype='ex'; $commune='Tempora/Epi1-0a.txt'; }
        let is_post_da = matches!(
            input.rubric,
            Rubric::Reduced1955 | Rubric::Rubrics1960
        );
        let in_former_epi_octave = matches!(input.date.month, 1)
            && (input.date.day < 13)
            && weekname.starts_with("Epi1")
            && dow != 0;
        if is_post_da && in_former_epi_octave && matches!(sancti_key.category, FileCategory::Sancti) {
            commune = Some(FileKey {
                category: FileCategory::Tempora,
                stem: "Epi1-0a".into(),
            });
            commune_type = CommuneType::Ex;
        }
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
        // Pick the commune slot that matches the active rubric, the
        // same way the kalendar-side resolver does. Pent02-1's
        // [Rank] block has:
        //   ;;Semiduplex II class;;5.6;;ex Tempora/Pent01-4
        //   (sed rubrica tridentina nisi rubrica cisterciensis)
        //   ;;Semiduplex IIS class;;2.9;;ex Tempora/Pent01-4
        //   (sed rubrica 196 aut rubrica 1955)
        //   ;;Feria;;1                       <- no commune column!
        //
        // Under R55/R60 the variant fires and the explicit-empty
        // commune sentinel propagates here — Pent02-1 winner has
        // NO commune chain, so proper-block falls through to the
        // Tempora-feria-Sunday-fallback (Pent02-0). Without this
        // dispatch the bare `commune` "ex Tempora/Pent01-4" leaked
        // through and Mass propers came from Corpus Christi.
        let (commune, commune_type) = match tempora_file {
            Some(f) => {
                let raw = pick_commune_for_rubric(f, input.rubric).unwrap_or_default();
                parse_commune_in_context(&raw, &tempora_key.category)
            }
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

#[allow(dead_code)] // Retained as a backstop for non-1570-kalendar
                    // dates; superseded for in-kalendar dates by
                    // resolve_sancti_for_tridentine_1570.
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
    mut trank: f32,
    mut srank: f32,
    rubric: Rubric,
    sancti_file: Option<&MassFile>,
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

    // Simplex demotion under 1955+: pre-1960 Simplex feasts (rank ≤ 1.1)
    // become commemorations only. Mirrors `horascommon.pl:456`:
    //   ($version =~ /19(?:55|6)|Monastic.*Divino/i && $srank[2] <= 1.1)
    // → `$sanctoraloffice = 0`.
    let is_simplex_demoted_rubric = matches!(
        rubric,
        Rubric::Reduced1955 | Rubric::Rubrics1960
    );
    if is_simplex_demoted_rubric && srank <= 1.1 {
        return false;
    }

    let temporal_name = tempora.officium.as_deref().unwrap_or("");
    let is_dominica = temporal_name.starts_with("Dominica");
    // Capture pre-adjustment trank so the Festum Domini elif (which
    // mirrors Perl's `$trank[2] <= 5` against the original rank, not
    // the Dominica-minor-downgraded one) can still fire on DA where
    // we'd otherwise have downgraded trank to 4.9.
    let trank_before_dominica_adjust = trank;

    // Dominica-minor rank handling — three regimes per
    // `horascommon.pl:422-433`:
    //
    //   Tridentine (1570/1888/1906): minor Sunday rank 4.3..5.0 → 2.9
    //     (any Duplex outranks).
    //   Divino Afflatu (1911 onwards): minor Sunday rank < 5.1 → 4.9
    //     (Duplex majus loses, Class II wins).
    //   Reduced 1955 + Rubrics 1960: leave the Sunday rank as-is. The
    //     1955/60 occurrence rules pull a different lever (Festum
    //     Domini gates) which lives below.
    let is_tridentine = matches!(rubric, Rubric::Tridentine1570 | Rubric::Tridentine1910);
    let is_divino = matches!(rubric, Rubric::DivinoAfflatu1911 | Rubric::Monastic);
    if is_tridentine && is_dominica && trank > 4.2 && trank < 5.1 {
        trank = 2.9;
    } else if is_divino && is_dominica && trank < 5.1 {
        trank = 4.9;
    }
    // "infra octavam Corp[oris Christi]" stays a 2.9 weekday for
    // every pre-1955 Roman rubric — see `horascommon.pl:425`.
    if (is_tridentine || is_divino)
        && temporal_name.contains("infra octavam Corp")
        && trank > 4.2
        && trank < 5.1
    {
        trank = 2.9;
    }

    // Adv/Quad srank cap (`horascommon.pl:441-444`):
    //   `} elsif ($dayname[0] =~ /Adv|Quad/ && $srank[2] > 6
    //              && $sname !~ /12-24/ && $saint{Rule} !~ /Patronus/) {
    //       $srank[2] = 6.01;
    //   }`
    // In Advent and Lent, a Class I sanctoral feast (rank > 6) is
    // capped to 6.01 so it can't outrank a Class I Sunday (rank 6.9
    // for Adv1, Quad1, Quad4, Pasc5, Pent01 default). Without this
    // cap, Annunciation (rank 6.5 from missa file, 6.92 from horas
    // file) would beat Quad4-0 Sunday (rank 6.9), but Perl gives
    // the Sunday + Annunciation-as-commemoration. Closes
    // T1910_Annunciation cluster (5 days). Christmas Eve (12-24)
    // and Patronus saints (parish patrons) are exempt.
    let temporal_is_adv_quad = temporal_name.contains("Quadragesim")
        || temporal_name.contains("Quadragesimam")
        || temporal_name.contains("Adventu")
        || temporal_name.contains("Adventum")
        || temporal_name.contains("Passion");
    // Christmas Eve exemption: Perl `horascommon.pl:443` says
    // `$sname !~ /12-24/`. Match the file stem directly via the
    // sancti name's "Vigilia Nativitatis" — Class I Vigil of
    // Christmas (Sancti/12-24, "In Vigilia Nativitatis Domini")
    // outranks Adv4 Sunday under R60. Drives R60_misc 19c
    // (2000-12-24 Sunday case).
    let sancti_lc = _sancti.name.to_lowercase();
    let sancti_is_12_24 = sancti_lc.contains("vigilia natalis")
        || sancti_lc.contains("vigilia nativitatis");
    let sancti_is_patronus = sancti_file
        .and_then(|sf| sf.sections.get("Rule"))
        .map(|r| r.to_lowercase().contains("patronus"))
        .unwrap_or(false);
    if temporal_is_adv_quad && srank > 6.0 && !sancti_is_12_24 && !sancti_is_patronus {
        srank = 6.01;
    }

    // "Festum Domini" exception (pre-1960): a Feast of the Lord with
    // rank ≥ 2 outranks ANY Sunday whose pre-adjustment rank ≤ 5.
    // Mirrors `horascommon.pl:477-481`:
    //   `} elsif ($trank[0] =~ /Dominica/i && ...
    //     elsif ($saint{Rule} =~ /Festum Domini/i && $srank[2] >= 2
    //     && $trank[2] <= 5) { $sanctoraloffice = 1; ... }`
    // Drives Sancti/09-14 (Exaltation of the Cross, Duplex majus
    // rank 4) outranking Pent14-18 Sunday under T1570/T1910/DA;
    // closes DA_SeptEmbersCross + R55_SeptEmbersCross. Must use the
    // PRE-Dominica-minor-adjustment trank or DA's trank=4.9 downgrade
    // would falsely satisfy `srank > trank` and bypass this branch.
    let is_pre_1960 = matches!(
        rubric,
        Rubric::Tridentine1570
            | Rubric::Tridentine1910
            | Rubric::DivinoAfflatu1911
            | Rubric::Reduced1955
            | Rubric::Monastic
    );
    if is_pre_1960
        && is_dominica
        && srank >= 2.0
        && trank_before_dominica_adjust <= 5.0
        && srank < trank_before_dominica_adjust
    {
        if let Some(sf) = sancti_file {
            if sf
                .sections
                .get("Rule")
                .map(|r| r.to_lowercase().contains("festum domini"))
                .unwrap_or(false)
            {
                return true;
            }
        }
    }

    // RG 15 (Rubricæ Generales 1960): the Immaculate Conception
    // outranks the II Sunday of Advent in occurrence. Mirrors Perl
    // `horascommon.pl:471-473`:
    //   `} elsif ($srank[0] =~ /Conceptione Immaculata/) {
    //       $sanctoraloffice = 1;
    //   }`
    // Without this exception the Adv/Quad srank cap above downgrades
    // Imm Conc (rank 6.5) to 6.01 — equal to Adv2-0's R60 rank 6.01
    // — and the strict `srank > trank` check leaves the Sunday as
    // winner. Closes R60_misc 12-08 case.
    if is_dominica {
        let sancti_name = sancti
            .map(|s| s.name.to_lowercase())
            .unwrap_or_default();
        // Match both word orders: missa-side `In Conceptione Immaculata
        // Beatæ Mariæ Virginis` AND kalendar-side `Immaculata
        // Conceptione Beatae Mariae Virginis` — Perl regex is just
        // `/Conceptione Immaculata/` against `$srank[0]` which is the
        // missa-side name, but our SanctiEntry's name comes from the
        // kalendar layer for non-1570 rubrics.
        if sancti_name.contains("conceptione immaculata")
            || sancti_name.contains("immaculata conceptione")
        {
            return true;
        }
    }

    // Sunday handling. Detect via the rendered name — pre-1960 Sundays
    // are written as `Dominica …`. (The Perl uses regex on `$trank[0]`
    // and `$dayname[0]`; we approximate with the officium string.)
    let is_sunday = is_dominica && trank >= 5.1;
    if is_sunday {
        // Class I Sundays (Adv1, Quad1-Quad4, Passion, Pasc5, Pent01)
        // have trank ≥ 6.0 — strict numeric comparison after the
        // Adv/Quad srank cap above; Class I sanctoral feasts in Lent
        // are capped to 6.01 and lose to the Class I Sunday at 6.9.
        if trank >= 6.0 {
            return srank > trank;
        }
        // Class II Sundays (regular post-Pent/post-Epi) — pre-1960:
        // Class I sanctoral (rank ≥ 6) outranks; lower sanctoral
        // commemorates only.
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

    // Apostolic-Vigil precedence rule (Tridentine 1570): Vigils of
    // Apostles (Andrew Nov 29, Thomas Dec 20) are CELEBRATED on
    // ADVENT Feria major days. Lenten ferias (Quad*), which encode
    // a higher actual privilege than Advent ferias despite sharing
    // the rank label "Feria major", still preempt the Vigil — Feb 23
    // (Vigil of Matthias) yields to the Quadragesimae feria. Advent
    // Quattuor Temporum (Ember days, also "Feria major") similarly
    // outrank the Apostolic Vigil — the Embertide is a privileged
    // class.
    let sancti_name = _sancti.name.as_str();
    let is_apostolic_vigil = sancti_name.starts_with("Vigilia")
        && (sancti_name.contains("Apostoli")
            || sancti_name.contains("Apostol")
            || sancti_name.contains("Apostolorum"));
    let is_advent_temporal = temporal_name.contains("Adventus")
        || temporal_name.contains("Advent")
        || temporal_name.contains("Hebdomadam I Adventus")
        || temporal_name.contains("Hebdomadam IV Adventus");
    let is_quattuor_temporum = temporal_name.contains("Quattuor Temporum");
    if is_apostolic_vigil
        && is_advent_temporal
        && !is_quattuor_temporum
        && srank >= 1.5
        && trank < 6.0
    {
        return true;
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

/// Downgrade post-1570 Octave-day Tempora ranks to feria *for rubrics
/// that predate the feast's institution*. The corpus carries elevated
/// `rank_num` for several feasts that were added to the calendar
/// after Trent — but the Tempora file is reused under all rubrics, so
/// we have to suppress the elevation for older rubrics.
///
///   * Sacred Heart (Sacratissimi Cordis): instituted 1856 (Pius IX).
///     Demote under T1570 only.
///   * Patrocinii Sancti Joseph: instituted 1847 (Pius IX).
///     Demote under T1570 only.
///   * Christ the King (Christi Regis): instituted 1925 (Pius XI,
///     *Quas primas*). Demote under T1570 + T1910.
///
/// Corpus Christi octave already existed in 1570, so no demotion for
/// `infra octavam Corporis Christi` — the Friday's Semiduplex II
/// classis rank is correct under every rubric.
fn downgrade_post_1570_octave(rank: f32, file: &MassFile, rubric: Rubric) -> f32 {
    let officium = file.officium.as_deref().unwrap_or("");

    // Sacred Heart and Patrocinii Joseph: pre-1856/1847 → only T1570
    // and Monastic 1617 demote.
    let is_pre_1856_demoter = matches!(rubric, Rubric::Tridentine1570 | Rubric::Monastic);
    let has_pre_1856_feast = officium.contains("Cordis Jesu")
        || officium.contains("Cordis Iesu")
        || officium.contains("Sacratissimi")
        // Match permissively — upstream is inconsistent about dot/
        // case ("Patrocinii St. Joseph", "Patrocínii"). See
        // UPSTREAM_WEIRDNESSES.md #4.
        || officium.contains("Patrocinii")
        || officium.contains("Patrocínii");
    if is_pre_1856_demoter && has_pre_1856_feast {
        return 1.0;
    }

    // Christ the King: pre-1925 → T1570, T1910 demote.
    let is_pre_1925_demoter = matches!(
        rubric,
        Rubric::Tridentine1570 | Rubric::Tridentine1910 | Rubric::Monastic
    );
    if is_pre_1925_demoter && officium.contains("Christi Regis") {
        return 1.0;
    }

    rank
}

/// Saturday Mass of the Blessed Virgin Mary (Tridentine 1570).
/// Mirrors `horascommon.pl:401-420`. Fires on free Saturdays
/// (DOW=6, no major temporal or sanctoral feast); the winner becomes
/// `Commune/C10[a|b|c|Pasc]` depending on season:
///
///   * Adv*    → `C10a`  (Common of BVM in Advent)
///   * Jan or Feb 1 → `C10b`  (Common of BVM after Christmas)
///   * Epi*/Quad* → `C10c`  (Common of BVM Epiphany–Septuagesima)
///   * Pasc*   → `C10Pasc` (Common of BVM during Eastertide)
///   * Otherwise → `C10`   (Common of BVM, generic)
///
/// Returns `None` when the conditions don't fire — including the
/// case where any of the higher-rank `<1.4` thresholds is exceeded.
fn saturday_bvm_winner_1570(
    dow: u32,
    weekname: &str,
    month: u32,
    day: u32,
    temporal_rank: f32,
    sanctoral_rank: f32,
) -> Option<FileKey> {
    if dow != 6 {
        return None;
    }
    if temporal_rank >= 1.4 || sanctoral_rank >= 1.4 {
        return None;
    }
    let stem = if weekname.starts_with("Adv") {
        "C10a"
    } else if month == 1 || (month == 2 && day == 1) {
        "C10b"
    } else if weekname.contains("Epi") || weekname.contains("Quad") {
        // Mirror Perl `horascommon.pl` ll. 1572 `($dayname[0] =~
        // /(Epi|Quad)/i)`: matches any substring `Epi` or `Quad` in
        // the week label, NOT just a prefix. The "PentEpi" labels
        // (long post-Pentecost cycle re-using Epi readings — Nov
        // 14/21 in 2026) use C10c for the Sat-BVM Mass.
        // Includes Quadp (pre-Lent / Septuagesima).
        "C10c"
    } else if weekname.starts_with("Pasc") {
        "C10Pasc"
    } else {
        "C10"
    };
    Some(FileKey {
        category: FileCategory::Commune,
        stem: stem.to_string(),
    })
}

/// September Embertide overlay (Tridentine 1570 only).
///
/// Mirrors the upstream `officestring()` overlay
/// (`SetupString.pl:720-779`): when the requested file is a Pent06+
/// or Epi week file AND the date maps to a `monthday`-derived stem
/// that exists in the missa corpus, prefer the monthday file.
///
/// The Perl implementation actually merges section bodies from the
/// monthday file onto the base Pent file (preserving `[Rank]` from
/// the base file). We side-step the merge by simply *replacing* the
/// base stem when the overlay applies, because for our corpus:
///
///   * Bodies under `093-X` are self-contained — the matching Pent
///     file (`Pent16-X` etc.) carries no Mass propers anyway.
///   * Ranks differ (Pent16-X is `Feria;;1`, 093-X is `Feria
///     major;;2.1`); using 093-X's rank is what keeps Saturday-BVM
///     from displacing Sept Ember Saturday.
///
/// For 1570 the only monthday files that ship under `missa/` are
/// `093-3`, `093-5`, `093-6` (Sept Ember Wed/Fri/Sat) and `104-0`
/// (Christ the King — post-1925, irrelevant to 1570). The function
/// is therefore a no-op outside those three dates.
fn apply_monthday_overlay_1570(
    base_stem: &str,
    day: u32,
    month: u32,
    year: i32,
    rubric: Rubric,
    corpus: &dyn Corpus,
) -> String {
    // Perl guard: `fname =~ /Pent|Epi/ && !/Pent0[1-5]/`.
    let is_pent06_plus = base_stem.starts_with("Pent")
        && !base_stem.starts_with("Pent01")
        && !base_stem.starts_with("Pent02")
        && !base_stem.starts_with("Pent03")
        && !base_stem.starts_with("Pent04")
        && !base_stem.starts_with("Pent05");
    let is_epi = base_stem.starts_with("Epi");
    if !is_pent06_plus && !is_epi {
        return base_stem.to_string();
    }
    // Modern-style monthday week numbering. Mirrors upstream
    // SetupString.pl:745:
    //   $monthday = monthday($day, $month, $year, ($version =~ /196/) + 0, $flag);
    // Only Rubrics 1960 ($version contains "196") gets the modern
    // formula. R55 ("Reduced - 1955") and earlier still use the
    // pre-1955 week count.
    let modernstyle = matches!(rubric, Rubric::Rubrics1960);
    let md = date::monthday(day, month, year, modernstyle, false);
    if md.is_empty() {
        return base_stem.to_string();
    }
    let candidate = FileKey {
        category: FileCategory::Tempora,
        stem: md.clone(),
    };
    // The monthday file must carry both [Officium] and [Rank] —
    // otherwise it's a partial-overlay file that the Perl runtime
    // would merge over the base while preserving the base's
    // [Rank] (e.g. `Tempora/104-0` ships only Commemoratio
    // sections under 1570 because Christ the King is post-1925
    // and the file is gated by `(rubrica 1960)` annotations). For
    // the 1570 baseline only the September Embertide files
    // (093-3, 093-5, 093-6) are full-overlay candidates.
    if let Some(file) = corpus.mass_file(&candidate) {
        if file.officium.is_some() && file.rank_num.is_some() {
            return md;
        }
    }
    base_stem.to_string()
}

/// Sunday-letter / Easter-coded Transfer table — temporal side.
///
/// Mirrors the upstream `Directorium::load_transfer` filtered to
/// `;;1570` entries with a `Tempora/...` main target. Two cases
/// drive this for our 1570 sweep:
///
///   * **Dominica anticipata** in years where Septuagesima falls
///     before some `Epi{n}-0` Sunday could land on a Sunday — the
///     Sunday Mass is then anticipated to the previous Saturday
///     (`01-31 = Tempora/Epi4-0tt` for letter d 2026).
///   * **Christmas-Sunday redirect** in years where 12-30 needs to
///     read the Sunday-Within-Octave Mass (`12-30 =
///     Tempora/Nat1-0` in the d.txt block).
///
/// Returns the new stem, or the original if no 1570-applicable
/// `Tempora/...` transfer fires today.
fn apply_transfer_temporal_1570(
    base_stem: &str,
    year: i32,
    month: u32,
    day: u32,
    rubric_tag: &str,
) -> String {
    let entries = crate::transfer_table::transfers_for(
        year, rubric_tag, month, day,
    );
    for entry in entries {
        // Only follow `Tempora/...` targets here. Sancti targets are
        // applied in `resolve_sancti_for_tridentine_1570`.
        if let Some(stem) = entry.main.strip_prefix("Tempora/") {
            return stem.to_string();
        }
    }
    // Inverse: if today's NATIVE temporal stem has been moved to
    // another date by a transfer rule, vacate today. Drives the
    // DA case `01-12=Tempora/Epi1-0;;DA` — when Jan 13 is Sunday,
    // Holy Family transfers to Jan 12 and Jan 13's calendar Sunday
    // position must be vacated so the kalendar's Sancti/01-13
    // (Octave of Epiphany / Baptism of the Lord) wins. Same
    // mechanism covers any future `mm-dd=Tempora/<stem>;;<rubric>`
    // entry on a non-today LHS for whichever rubric is active.
    if crate::transfer_table::temporal_stem_moved_elsewhere(
        year, rubric_tag, month, day, base_stem,
    ) {
        // Use a sentinel stem that doesn't resolve to a Tempora
        // file — falls back through to the kalendar/sancti winner.
        return String::new();
    }
    base_stem.to_string()
}

/// Sunday-letter / Easter-coded Transfer table — sancti side.
///
/// Same source as `apply_transfer_temporal_1570`; this branch fires
/// for bare-stem targets (e.g. `11-28 = 11-29` → Vigil of Andrew
/// transferred from Sunday Nov 29 to Saturday Nov 28). Returns the
/// transferred sancti stem and rank when applicable.
fn apply_transfer_sancti_1570(
    year: i32,
    month: u32,
    day: u32,
    rubric_tag: &str,
    corpus: &dyn Corpus,
    rubric: Rubric,
) -> Option<(String, f32)> {
    // Iterate transfer entries in REVERSE so easter-file rules
    // (loaded second) win over letter-file rules (loaded first).
    // Mirrors Perl `Directorium::load_transfers` line 135-137 where
    // the second `push` to `@lines` overrides the first via later
    // hash insertion. Drives 2032-04-06 T1910: c.txt has
    // `04-06=04-04;;1888 1906` (Isidore), 328.txt has
    // `04-06=03-21;;1570 1888 1906` (Benedict). Easter file wins
    // → Benedict.
    let entries = {
        let mut e = crate::transfer_table::transfers_for(
            year, rubric_tag, month, day,
        );
        e.reverse();
        e
    };
    for entry in entries {
        // Skip Tempora-targeted entries (handled by
        // `apply_transfer_temporal_1570`).
        if entry.main.starts_with("Tempora/") {
            continue;
        }
        // Skip the no-op marker `X-X` (used for "this date deferred,
        // nothing happens"; cf. d.txt 08-23 in 1570).
        if entry.main == "X-X" {
            continue;
        }
        // `xx-yy=11-29v` (Andrew's Vigil transferred when 11-30 falls
        // on Sunday) and `10-30=10-31v` (All Saints Vigil transferred
        // off a Sunday Oct 31) reference a `<stem>v` file that doesn't
        // physically exist — Perl strips the `v` and uses the bare
        // file. Walk through `resolve_sancti_stem` first so the rank
        // lookup hits the actual on-disk Sancti/<stem>.txt.
        let resolved_key = resolve_sancti_stem(&entry.main, corpus);
        let metadata_key = effective_tempora_key(&resolved_key, corpus);
        let mass = corpus.mass_file(&metadata_key);
        // Rubric-aware rank pick. Joseph (Sancti/03-19) carries
        // `rank_num_1570 = 3.0` (Duplex), `rank_num = 6.1`
        // (1888-elevated Duplex II classis), `rank_num_1960 = 6.0`
        // (R60 Class I). The previous `rank_num_1570.or(rank_num)`
        // pick mirrored T1570 only and lost the elevation under
        // R60 — Joseph transferred to Quad2-1 Mon under R60 should
        // win at rank 6.0 (vs feria 3.9), not 3.0. Closes R60_misc
        // 03-20 day.
        let rank = mass
            .and_then(|m| match rubric {
                Rubric::Tridentine1570 | Rubric::Monastic => {
                    m.rank_num_1570.or(m.rank_num)
                }
                Rubric::Tridentine1910 => m.rank_num_1906.or(m.rank_num),
                Rubric::DivinoAfflatu1911 => m.rank_num,
                Rubric::Reduced1955 => m.rank_num_1955.or(m.rank_num),
                Rubric::Rubrics1960 => m
                    .rank_num_1960
                    .or(m.rank_num_1955)
                    .or(m.rank_num),
            })
            .unwrap_or(0.0);
        return Some((entry.main, rank));
    }
    None
}

/// Pick the rubric-specific variant of a Tempora stem when one
/// exists. Mirrors upstream `Directorium::load_tempora`, which reads
/// `Tabulae/Tempora/Generale.txt` and keeps only the rules whose
/// rubric-token column matches the active version's `transfer`
/// token (per `Tabulae/data.txt`).
///
/// Examples:
///   * `Epi1-0`   under 1570/1888/1906 → `Epi1-0a` (post-1911 Holy
///                Family bumps the 1570 Dominica infra Octavam off
///                the bare slot)
///   * `Quad3-3`  under 1570 only       → `Quad3-3t` (T1910 keeps the
///                bare Lectio; 1955+ also keeps it)
///   * `Pent02-5` under 1888/1906       → `Pent02-5o` (Sacred Heart);
///                under 1570            → `Pent02-5Feria` (no Sacred
///                Heart yet — feria after Corpus Christi octave).
///
/// Trinity Sunday (`Pent01-0`) has no entry in the table — it
/// already existed in 1570 — so the bare stem applies under all
/// rubrics.
/// Read the Sancti file's `commune` field with rubric-aware
/// preference. Mirrors the dispatch used in
/// `resolve_sancti_for_tridentine_1570`'s kalendar-lookup branch
/// (the per-bucket `commune_*` accessors). Used by both the
/// upstream-Transfer-table path and the heuristic walk-back.
fn pick_commune_for_rubric(m: &MassFile, rubric: Rubric) -> Option<String> {
    match rubric {
        Rubric::Tridentine1570 => m.commune_1570.clone().or_else(|| m.commune.clone()),
        Rubric::Tridentine1910 => m.commune_1906.clone().or_else(|| m.commune.clone()),
        Rubric::DivinoAfflatu1911 => m.commune.clone().or_else(|| m.commune_1570.clone()),
        // R55: prefer year-specific 1955 variant, then SP override
        // (Gregory the Great 03-12 has `(sed communi Summorum
        // Pontificum)` switching commune from C4a to C4b under
        // R55), then default.
        Rubric::Reduced1955 => m
            .commune_1955
            .clone()
            .or_else(|| m.commune_sp.clone())
            .or_else(|| m.commune.clone())
            .or_else(|| m.commune_1570.clone()),
        // R60: 1960 > 1955 > SP > default. R60 typically has its
        // own `(sed rubrica 196)` body that shadows SP.
        Rubric::Rubrics1960 => m
            .commune_1960
            .clone()
            .or_else(|| m.commune_1955.clone())
            .or_else(|| m.commune_sp.clone())
            .or_else(|| m.commune.clone())
            .or_else(|| m.commune_1570.clone()),
        Rubric::Monastic => m.commune_1570.clone().or_else(|| m.commune.clone()),
    }
}

/// Sister of `pick_commune_for_rubric` for the `officium` field.
fn pick_officium_for_rubric(m: &MassFile, rubric: Rubric) -> Option<String> {
    match rubric {
        Rubric::Tridentine1910 => m.officium_1906.clone().or_else(|| m.officium.clone()),
        Rubric::Reduced1955 => m.officium_1955.clone().or_else(|| m.officium.clone()),
        Rubric::Rubrics1960 => m
            .officium_1960
            .clone()
            .or_else(|| m.officium_1955.clone())
            .or_else(|| m.officium.clone()),
        // T1570 / DA / Monastic: no canonical override slot stored —
        // the parser keeps the bare `officium` for these. The 1570
        // bucket only holds rank/commune deltas (no name change).
        _ => m.officium.clone(),
    }
}

/// Sister of `pick_commune_for_rubric` for the `rank` (class label)
/// field.
fn pick_rank_class_for_rubric(m: &MassFile, rubric: Rubric) -> Option<String> {
    match rubric {
        Rubric::Tridentine1910 => m.rank_1906.clone().or_else(|| m.rank.clone()),
        Rubric::Reduced1955 => m.rank_1955.clone().or_else(|| m.rank.clone()),
        Rubric::Rubrics1960 => m
            .rank_1960
            .clone()
            .or_else(|| m.rank_1955.clone())
            .or_else(|| m.rank.clone()),
        _ => m.rank.clone(),
    }
}

fn pick_tempora_variant(stem: &str, rubric: Rubric, corpus: &dyn Corpus) -> String {
    if let Some(target) = crate::tempora_table::redirect(stem, rubric) {
        let key = FileKey {
            category: FileCategory::Tempora,
            stem: target.to_string(),
        };
        if corpus.mass_file(&key).is_some() {
            return target.to_string();
        }
    }
    stem.to_string()
}

/// Backward-compat shim: equivalent to `pick_tempora_variant(stem,
/// Rubric::Tridentine1570, corpus)`. Used by the precedence-side
/// `decide_sanctoral_wins_1570` until that pathway gets the rubric
/// threaded through.
fn pick_tempora_variant_for_1570(stem: &str, corpus: &dyn Corpus) -> String {
    pick_tempora_variant(stem, Rubric::Tridentine1570, corpus)
}

/// Transferred-feast lookup. Walk back up to 6 days. For each day,
/// check if the kalendar 1570 entry there was preempted by its
/// own day's temporal cycle (e.g. Sunday outranking a Semiduplex
/// feast). If yes, AND the days between that one and `day` are all
/// "free" (no kalendar entry that would itself need a feast slot),
/// return the bumped feast — it transfers to `day`.
///
/// This is a coarse approximation of the Tridentine transfer rules.
/// We don't track the year-level transfer table; instead we re-derive
/// transfers per-date by scanning recent kalendar entries. Only
/// catches the simple "Sunday bumps a Semiduplex" pattern; complex
/// chains of transfers are not yet handled.
fn transferred_sancti_for_1570(
    year: i32,
    month: u32,
    day: u32,
    layer: crate::kalendaria_layers::Layer,
    corpus: &dyn Corpus,
) -> Option<(String, String, f32)> {
    // Walk back up to 6 days looking for the most recent kalendar
    // entry. If found, check if it was preempted on its native date.
    // If yes, scan FORWARD from that date and apply the transfer to
    // the FIRST free day — i.e. only fire the transfer once.
    let (mut cursor_y, mut cursor_m, mut cursor_d) = (year, month, day);
    for _ in 0..6 {
        let prev = previous_calendar_day(cursor_y, cursor_m, cursor_d);
        cursor_y = prev.0;
        cursor_m = prev.1;
        cursor_d = prev.2;
        // Layer-aware kalendar key: for the back-walk we only suppress
        // leap-year Feb 23 on Pius1570 (where 02-23 main is the Vigil
        // and the leap shift moves it). Under PiusX1906+ the main is
        // Petri Damiani, which is NOT moved by the bissextile shift —
        // we must walk through real Feb 23 leap year to find Petri
        // when she was preempted by Quad1-0 Sunday.
        let (look_m, look_d) = if matches!(layer, crate::kalendaria_layers::Layer::Pius1570) {
            let Some(k) = date::sancti_kalendar_key(cursor_y, cursor_m, cursor_d) else {
                continue;
            };
            k
        } else {
            date::sday_pair(cursor_m, cursor_d, cursor_y)
        };
        let Some(entry) = kalendarium_1570::lookup_for_layer(layer, look_m, look_d) else {
            continue;
        };
        // Octave-day saints ("Septima die infra Octavam ...", etc.)
        // don't transfer in 1570 — they're commemorated or lost when
        // preempted, never moved. Only fixed-date proper saints
        // transfer. Heuristic match on the kalendar's saint-name.
        if is_octave_day_kalendar_name(&entry.main.name) {
            return None;
        }
        // Tridentine 1570 rule: only Duplex+ feasts transfer when
        // preempted. Simplex (1.x) and Semiduplex (2.x) saints are
        // commemorated under the higher-ranking office and lost when
        // preempted; they don't move forward to the next free day.
        // Without this guard, Louis of France (Aug 25, rank 2.2)
        // displaced by a Sunday wrongly lands on Aug 26 and bumps
        // Zephyrinus instead of being commemorated.
        if entry.main.rank_num < 3.0 {
            return None;
        }
        // Was this kalendar entry preempted on its native date?
        let was_preempted = was_sancti_preempted_1570(
            cursor_y, cursor_m, cursor_d, entry, layer, corpus,
        );
        if !was_preempted {
            // Prior saint occupied its own day — keep walking back
            // through it (the forward walk will respect blocking).
            continue;
        }
        // Walk forward from the bumped date. At each subsequent day:
        //   * If the day is free (no native saint) AND the temporal
        //     allows, the transferred saint lands here.
        //   * If the day has a native saint of LOWER rank than the
        //     transferred saint, the transfer displaces it (native
        //     becomes a commemoration).
        //   * Otherwise (saint of equal/higher rank, or temporal still
        //     outranks) keep walking.
        let (mut walk_y, mut walk_m, mut walk_d) = (cursor_y, cursor_m, cursor_d);
        for _ in 0..14 {
            let next = next_calendar_day(walk_y, walk_m, walk_d);
            walk_y = next.0;
            walk_m = next.1;
            walk_d = next.2;
            // Stop if we've walked further than `day` — the saint
            // was claimed by an earlier free day.
            if (walk_y, walk_m, walk_d) > (year, month, day) {
                return None;
            }
            // Is this day's slot available (free or lower-ranked saint)?
            let native_here = match date::sancti_kalendar_key(walk_y, walk_m, walk_d) {
                Some((look_m2, look_d2)) => {
                    kalendarium_1570::lookup_for_layer(layer, look_m2, look_d2)
                }
                None => None,
            };
            // Tridentine practice: a transferred saint can displace a
            // SIMPLEX (rank < 2) native saint, who is then
            // commemorated. Semiduplex+ native saints (rank 2.0+)
            // outrank the transferred saint and block it (the
            // transferred saint walks further or gets lost).
            // Without this guard, Leo I (Apr 11, rank 3) preempted by
            // the Easter Octave transfers forward and wrongly displaces
            // Hermenegildi (Apr 13, rank 2.2), instead of being
            // commemorated and walking past.
            let blocked = match native_here {
                None => false, // free
                Some(e) => e.main.rank_num >= 2.0,
            };
            if blocked {
                continue; // walk further — this day claims its native
            }
            // Privileged-Octave block: the Tridentine 1570 heuristic
            // transfer rule only fires when there's a genuinely free
            // day for the displaced Duplex. The Octave of Ascension
            // (Pasc5-x weekdays + Pasc6-0 Sunday in Octava) is a
            // privileged Octave under 1570 — Perl's `Directorium`
            // does NOT walk transferred Sancti into it (1940-05-05
            // and 2035-05-05 were the residual fail-pattern). The
            // Tridentine downgrade of Pasc6-0 from rank 5 to 2.9
            // (Dominica minor) made the rank-only check land
            // Athanasius (rank 3) on the Sunday in Octava.
            // Block the walk by Tempora-name match — privileged
            // Octaves carry "Octavam Ascensionis" or
            // "infra Octavam Ascensionis" in their officium.
            let walk_weekname = date::getweek(walk_d, walk_m, walk_y, false, true);
            let walk_is_pasc5_or_pasc6 = walk_weekname == "Pasc5"
                || walk_weekname == "Pasc6";
            if walk_is_pasc5_or_pasc6 {
                let walk_dow = date::day_of_week(walk_d, walk_m, walk_y);
                let walk_stem_default = if walk_dow == 0 {
                    format!("{}-0", walk_weekname)
                } else {
                    format!("{}-{}", walk_weekname, walk_dow)
                };
                let walk_stem = pick_tempora_variant_for_1570(
                    &walk_stem_default, corpus,
                );
                let walk_key = FileKey {
                    category: FileCategory::Tempora,
                    stem: walk_stem,
                };
                let walk_eff = effective_tempora_key(&walk_key, corpus);
                let walk_tname = corpus
                    .mass_file(&walk_eff)
                    .and_then(|f| f.officium.as_deref())
                    .unwrap_or("");
                if walk_tname.contains("Ascensionis") {
                    continue;
                }
            }
            // Slot available. Does the temporal still preempt the
            // transferred saint here?
            if was_sancti_preempted_1570(walk_y, walk_m, walk_d, entry, layer, corpus) {
                continue; // walk further
            }
            // Saint can land here.
            if (walk_y, walk_m, walk_d) == (year, month, day) {
                return Some((
                    entry.main.stem.clone(),
                    entry.main.name.clone(),
                    entry.main.rank_num,
                ));
            }
            // The saint settles on an earlier free day; today is
            // unaffected.
            return None;
        }
        return None;
    }
    None
}

/// True when the kalendar saint-name is for an octave-day entry that
/// shouldn't transfer when preempted ("Die II infra Octavam ...",
/// "Septima die infra Octavam ...", "Octavae Ss. ..."). Only proper
/// (fixed-date) saints transfer in 1570.
fn is_octave_day_kalendar_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    // Octave-related forms (don't transfer in 1570).
    if lower.contains("infra octavam")
        || lower.contains("octava ")
        || lower.contains("die infra")
        || lower.contains("octavae ")
        || lower.starts_with("die ")
        || lower.starts_with("octava")
        || lower.starts_with("octavae")
    {
        return true;
    }
    // Vigils are tied to the day before their saint — they don't
    // transfer either; if preempted they're lost or shift to a
    // different rule. Match both "vigilia" and "vigilae" forms.
    if lower.starts_with("vigil") || lower.contains(" vigil") {
        return true;
    }
    false
}

fn previous_calendar_day(year: i32, month: u32, day: u32) -> (i32, u32, u32) {
    if day > 1 {
        return (year, month, day - 1);
    }
    if month > 1 {
        let prev_month = month - 1;
        let last_day = match prev_month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => if crate::date::leap_year(year) { 29 } else { 28 },
            _ => 30,
        };
        return (year, prev_month, last_day);
    }
    // Jan 1 → Dec 31 of previous year.
    (year - 1, 12, 31)
}

fn next_calendar_day(year: i32, month: u32, day: u32) -> (i32, u32, u32) {
    let last_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if crate::date::leap_year(year) { 29 } else { 28 },
        _ => 30,
    };
    if day < last_day {
        return (year, month, day + 1);
    }
    if month < 12 {
        return (year, month + 1, 1);
    }
    (year + 1, 1, 1)
}

/// Did the given kalendar entry get preempted by its own day's
/// temporal under Tridentine 1570 rules? Coarse: check if the
/// temporal-side rank for that day exceeds the entry's rank.
fn was_sancti_preempted_1570(
    year: i32,
    month: u32,
    day: u32,
    entry: &kalendarium_1570::Entry1570,
    _layer: crate::kalendaria_layers::Layer,
    corpus: &dyn Corpus,
) -> bool {
    use crate::date;
    let weekname = date::getweek(day, month, year, false, true);
    let dow = date::day_of_week(day, month, year);
    let tempora_stem_default = if dow == 0 {
        format!("{}-0", weekname)
    } else {
        format!("{}-{}", weekname, dow)
    };
    let tempora_stem = pick_tempora_variant_for_1570(&tempora_stem_default, corpus);
    // Same Sept Embertide overlay as compute_occurrence, so the
    // preemption check sees Pent16-6's rank as 2.1 (from 093-6) on
    // Sept Ember Saturday rather than the bare 1.0 of Pent16-6.
    // The transfer-eligibility check operates on the 1570 baseline,
    // so use Tridentine1570 for the modern-style flag.
    let tempora_stem = apply_monthday_overlay_1570(
        &tempora_stem, day, month, year, Rubric::Tridentine1570, corpus,
    );
    let tempora_key = FileKey {
        category: FileCategory::Tempora,
        stem: tempora_stem,
    };
    let effective_key = effective_tempora_key(&tempora_key, corpus);
    let tempora_file = corpus.mass_file(&effective_key);
    let mut trank = tempora_file
        .and_then(|f| f.rank_num)
        // The transfer-eligibility check operates on the 1570 baseline
        // (was the feast bumped *under 1570*?). Always demote the
        // post-1570 octaves here, regardless of the active rubric.
        .map(|r| downgrade_post_1570_octave(r, tempora_file.unwrap(), Rubric::Tridentine1570))
        .unwrap_or(0.0);
    // Mirror the `decide_sanctoral_wins_1570` precedence model so the
    // transfer-or-not decision matches what compute_office actually
    // does on the saint's native date. In particular, "Dominica
    // minor" Sundays (rank 4.2..5.1) get downgraded to 2.9 — a Duplex
    // saint (rank 3.0) outranks them and ISN'T preempted.
    let temporal_name = tempora_file
        .and_then(|f| f.officium.as_deref())
        .unwrap_or("");
    let is_dominica = temporal_name.starts_with("Dominica");
    if is_dominica && trank > 4.2 && trank < 5.1 {
        trank = 2.9;
    }
    // Octave-of-Corpus-Christi same downgrade.
    if temporal_name.contains("infra octavam Corp") && trank > 4.2 && trank < 5.1 {
        trank = 2.9;
    }
    // Saint is preempted iff effective trank > srank. (Don't apply
    // the "Sunday wins ties" rule yet — it's rare enough at the
    // transfer layer that we skip it for now.)
    //
    // Use the *higher* of the kalendar rank and the sancti-corpus rank.
    // Kalendar 1570 stores integer ranks (2 = Semiduplex), while the
    // sancti corpus has the fractional convention (2.2 for Semiduplex).
    // Without taking the max, days like Sept 16 (Cornelius/Cyprian:
    // kalendar=2.0, corpus=2.2) report preempted by Sept Embertide
    // Wednesday (rank 2.1 from monthday overlay) when in fact the
    // saint outranks the embertide.
    let (look_m, look_d) = date::sday_pair(month, day, year);
    // Pick the era-specific rank from the Sancti corpus. Many saints
    // were promoted post-Tridentine (Annunciation: 1570 rank 5.0
    // Duplex II classis, default 6.92 Duplex I classis), and the
    // preemption test must use the era's actual rank.
    //
    // Layer-aware preference:
    //   - Pius1570 layer ⇒ prefer the `1570` rubric variant.
    //   - Any later layer ⇒ prefer `default` (post-Tridentine
    //     elevations match Pius X / Pius XI / Pius XII reality);
    //     fall through to `1570` only when no default exists.
    let entries = corpus.sancti_entries(look_m, look_d);
    let prefer_1570 =
        matches!(_layer, crate::kalendaria_layers::Layer::Pius1570);
    let corpus_rank = if prefer_1570 {
        entries
            .iter()
            .find(|e| e.rubric.contains("1570"))
            .or_else(|| entries.iter().find(|e| e.rubric == "default"))
    } else {
        entries
            .iter()
            .find(|e| e.rubric == "default")
            .or_else(|| entries.iter().find(|e| e.rubric.contains("1570")))
    }
    .and_then(|e| e.rank_num)
    .unwrap_or(0.0);
    let srank = entry.main.rank_num.max(corpus_rank);
    trank > srank
}

/// Walk a file's `parent` chain, returning the first key whose
/// file actually carries `[Rank]` data. Lets the occurrence layer
/// see-through redirect-only files like `Tempora/Adv1-0o` (a single
/// `@Tempora/Adv1-0` line) or `Sancti/05-09t` (a single
/// `@Sancti/05-09` line) and pick up the rank from the parent.
/// Stops after 4 hops as a defensive cycle break.
fn effective_tempora_key(key: &FileKey, corpus: &dyn Corpus) -> FileKey {
    let mut k = key.clone();
    for _ in 0..4 {
        let Some(file) = corpus.mass_file(&k) else {
            return k;
        };
        if file.rank_num.is_some() || file.officium.is_some() {
            return k;
        }
        let parent = file.parent_1570.as_deref().or(file.parent.as_deref());
        match parent {
            Some(p) => k = FileKey::parse(p),
            None => return k,
        }
    }
    k
}

/// Resolve a file's effective Officium, chasing the file-level
/// `parent` inherit when the local file is body-less (e.g.
/// `Sancti/01-12t.txt = @Tempora/Epi1-0a` has no [Officium] of its
/// own; the parent Tempora/Epi1-0a supplies it).
fn effective_officium(key: &FileKey, corpus: &dyn Corpus) -> Option<String> {
    let mut k = key.clone();
    for _ in 0..4 {
        let file = corpus.mass_file(&k)?;
        if let Some(o) = file.officium.as_deref() {
            return Some(o.to_string());
        }
        let parent = file.parent_1570.as_deref().or(file.parent.as_deref());
        if let Some(p) = parent {
            k = FileKey::parse(p);
            continue;
        }
        break;
    }
    None
}

/// When the override stem points at a "Dominica" Mass file but the
/// actual date is NOT a Sunday this year, swap to the numerical-day
/// variant. Concretely: 01-12 in 2026 is Monday; the kalendar table
/// assumes Sunday and points at Sancti/01-12t (Dominica infra
/// Octavam) — instead use Sancti/01-12 (Septima die infra Octavam).
fn redirect_dominica_to_numerical(
    stem: &str,
    year: i32,
    month: u32,
    day: u32,
    corpus: &dyn Corpus,
) -> String {
    let key = FileKey {
        category: FileCategory::Sancti,
        stem: stem.to_string(),
    };
    let officium_is_dominica = effective_officium(&key, corpus)
        .map(|o| o.trim_start().to_lowercase().starts_with("dominica"))
        .unwrap_or(false);
    if !officium_is_dominica {
        return stem.to_string();
    }
    if date::day_of_week(day, month, year) == 0 {
        return stem.to_string(); // Actually Sunday — keep.
    }
    // Strip the trailing `t` (the most common Sunday-only suffix)
    // and check if the bare stem exists. For other suffix forms,
    // try just dropping the suffix letter.
    for trim in [1, 2] {
        if stem.len() > trim {
            let bare = &stem[..stem.len() - trim];
            let bare_key = FileKey {
                category: FileCategory::Sancti,
                stem: bare.to_string(),
            };
            if corpus.mass_file(&bare_key).is_some() {
                return bare.to_string();
            }
        }
    }
    stem.to_string()
}

/// Resolve a Sancti stem to a FileKey backed by an actual MassFile.
/// When the requested stem points to a body-less file (typically a
/// single-line `@Sancti/<base>` redirect like `12-24o.txt`), fall
/// through to the base stem — the redirect target carries the body.
fn resolve_sancti_stem(stem: &str, corpus: &dyn Corpus) -> FileKey {
    let key = FileKey {
        category: FileCategory::Sancti,
        stem: stem.to_string(),
    };
    if corpus.mass_file(&key).is_some() {
        return key;
    }
    // Common `@`-only redirect: stem ends in a one-letter suffix
    // (`o`/`t`/`r`/`s`) and the bare base file exists.
    for trim in [1, 2] {
        if stem.len() > trim {
            let base = &stem[..stem.len() - trim];
            let candidate = FileKey {
                category: FileCategory::Sancti,
                stem: base.to_string(),
            };
            if corpus.mass_file(&candidate).is_some() {
                return candidate;
            }
        }
    }
    key
}

/// Resolve the sanctoral side for Tridentine 1570: applies the
/// `kalendarium_1570.txt` override if present (selecting the right
/// Tridentine variant of the Sancti file, e.g. `01-23 → 01-23o`),
/// otherwise falls through to the post-1570 corpus default.
///
/// Returns `(file_key, sancti_entry)` — the entry's `rank_num` and
/// `commune` are wired downstream by the precedence logic.
fn resolve_sancti_for_tridentine_1570(
    year: i32,
    month: u32,
    day: u32,
    layer: crate::kalendaria_layers::Layer,
    rubric: Rubric,
    corpus: &dyn Corpus,
) -> (FileKey, Option<SanctiEntry>) {
    // Sunday-letter / Easter-coded Transfer table override (sancti
    // side). Highest precedence — entries like `11-28 = 11-29`
    // (Vigil of Andrew anticipated to Saturday) and `09-19 =
    // 09-20o` (Vigil of Matthew anticipated when Matthew falls on
    // Sunday) live here and override both the kalendar and the
    // walked-back transfer-of-preempted-saints chain.
    // Use the date's kalendar key (not the real date) for transfer-
    // table lookup. Under leap years on real Feb 24 (kalendar 02-29),
    // the rule `02-29=02-22~02-23o;;1888 1906` is keyed on 02-29 and
    // resolves the day's saint to Cathedra Petri (target.main 02-22)
    // with Vigil-of-Matthias as commemoration extra. Closes T1910 +
    // DA leap-year Feb-24 cases (1976, 1996, 2032).
    let (transfer_m, transfer_d) = match date::sancti_kalendar_key(year, month, day) {
        Some((m, d)) => (m, d),
        None => (month, day),
    };
    if let Some((stem, rank_num)) =
        apply_transfer_sancti_1570(year, transfer_m, transfer_d, rubric.transfer_rubric_tag(), corpus, rubric)
    {
        let key = resolve_sancti_stem(&stem, corpus);
        let metadata_key = effective_tempora_key(&key, corpus);
        let mass = corpus.mass_file(&metadata_key);
        let name = mass
            .and_then(|m| pick_officium_for_rubric(m, rubric))
            .unwrap_or_else(|| stem.clone());
        let commune = mass
            .and_then(|m| pick_commune_for_rubric(m, rubric))
            .unwrap_or_default();
        let rank_class = mass
            .and_then(|m| pick_rank_class_for_rubric(m, rubric))
            .unwrap_or_default();
        let entry = SanctiEntry {
            rubric: "transfer-table".into(),
            name,
            rank_class,
            rank_num: Some(rank_num),
            commune,
        };
        return (key, Some(entry));
    }
    // Transferred-feast lookup: scan back up to 6 days for a Sancti
    // that was preempted on its native date by a higher-ranked
    // Sunday/feast. Apply when:
    //   1. Today has no native kalendar entry, OR
    //   2. Today's native entry has lower rank than the transferred
    //      saint (the transferred saint then displaces today's native
    //      saint, who gets commemorated).
    // Leap-year Feb-23 suppression (`sancti_kalendar_key`) only applies
    // to layers where Feb 23's main is the Vigil-of-Matthias (`02-23o`).
    // Under Pius1570 / Monastic the Vigil IS the main → suppress in
    // leap years (the Vigil moves to real Feb 24 = kalendar 02-29).
    // Under PiusX1906+ the main is `02-23r` (Petri Damiani, Duplex 3),
    // which is NOT moved by the bissextile shift — so we keep the
    // entry. Closes Quadp_Quad_Commune_C4a leap-year days.
    let kalendar_key = if matches!(layer, crate::kalendaria_layers::Layer::Pius1570) {
        date::sancti_kalendar_key(year, month, day)
    } else {
        Some(date::sday_pair(month, day, year))
    };
    let (look_m, look_d) = kalendar_key.unwrap_or_else(|| date::sday_pair(month, day, year));
    // If the year's transfer table ANNOUNCES that this date's stem
    // (e.g. `03-25` = Annunciation) has been moved to a future date
    // (`04-08=03-25` for years where Easter falls on March 31), AND
    // the native saint was actually preempted on its native date, the
    // saint is suppressed on its native date — fall through to the
    // temporal cycle. Mirrors upstream's directorium behaviour:
    // March 25 in 2024 (Holy Monday) becomes Quad6-1, not Annunciation.
    //
    // The preemption guard matters because table entries like
    // `02-03=02-01` (1570, letter d) are *conditional* — they only
    // fire in years where 02-01's saint is genuinely preempted. In
    // 2020 (letter d) Ignatius (Feb 1, rank 2.2) lands on a free
    // Saturday and isn't preempted, so the Feb-3 transfer entry
    // should not activate.
    //
    // `kalendar_key.is_none()` covers the leap-year Feb-23 suppression
    // (the Vigil of Matthias slid to real Feb 24 = kalendar 02-29);
    // real Feb 23 in leap years has no kalendar entry, falls through
    // to ferial.
    let kalendar_entry_for_date = kalendar_key.and_then(|(m, d)| {
        kalendarium_1570::lookup_for_layer(layer, m, d)
    });
    // Perl's `Directorium::transfered()` consults the year's explicit
    // Sunday-letter Transfer table and returns TRUE whenever the
    // current date's stem is moved by an explicit rule. The
    // suppression is unconditional — Perl does NOT also gate on a
    // preemption check. Earlier the Rust port wrapped this in
    // `was_sancti_preempted_1570(...)` (a heuristic that asks whether
    // the saint would have lost on rank to the Tempora) which worked
    // for the most common cases (Annunciation in Holy Week — saint
    // does lose on rank) but blocked the c-letter `04-12=04-11` rule
    // for 2027-04-11 (St. Leo would *win* on rank against Pasc2-0
    // Dominica minor 2.9 < Duplex 3.0, so the heuristic refused to
    // suppress, but Perl's explicit table moves him anyway). Drop
    // the gate — the upstream letter file already encodes which
    // saints move and which don't, so the rank check is redundant
    // and counter-productive.
    // Collect the stems that the kalendar entry could match against
    // a transfer rule's extras (e.g. `02-23o` for the Vigil of
    // Matthias on the bissextile shift). Without this, the
    // date-target match alone misses leap-year Feb 24, where mm_dd =
    // "02-29" and the rule is `02-23=02-22~02-23o` (target is "02-22",
    // extras has "02-23o" — only the stem mention catches it).
    // Only the MAIN stem matters for "is this date's office transferred
    // away?" — when a transfer rule moves a commemoration stem (e.g.
    // `02-29=02-22~02-23o;;1888 1906` moves the Vigil-of-Matthias
    // commemoration off real Feb 24 leap), the date's MAIN feast (Petri
    // Damiani 02-23r on 1981-02-23 T1910) stays put. Without this
    // narrowing, the date-target heuristic was nuking the whole 02-23
    // entry on every non-leap T1910 February-23 because the rule
    // mentions `02-23o` in its extras column. Closes
    // Quadp_Quad_Commune_C4a (29 days).
    let candidate_main_stems: Vec<&str> = kalendar_entry_for_date
        .iter()
        .map(|e| e.main.stem.as_str())
        .collect();
    let suppressed_by_transfer =
        crate::transfer_table::stem_transferred_away_with_stems(
            year, rubric.transfer_rubric_tag(), look_m, look_d, &candidate_main_stems,
        );
    let native_entry = if suppressed_by_transfer {
        None
    } else {
        kalendar_entry_for_date
    };
    let native_rank = native_entry.map(|e| e.main.rank_num).unwrap_or(0.0);
    // The heuristic transfer-walk simulates Tridentine 1570
    // transfer rules ("displaced Duplex+ moves to next free day").
    // Post-1570 rubrics use the upstream Transfer tables for
    // explicit per-rubric transfers — falling back to the
    // heuristic would wrongly transfer (e.g.) Ignatius from Feb 1
    // to Feb 3 under T1910, where Perl keeps Blasius. Restrict the
    // heuristic to rubrics that share the 1570/M1617 transfer
    // semantics.
    // T1910 fallback: enable the heuristic walk ONLY when the date has
    // NO native kalendar entry, OR when the only native entry is a Vigil
    // (rank class "Vigilia") that itself was moved away by a transfer
    // rule (e.g. real Feb 24 leap year, kalendar 02-29 = Vigil-of-
    // Matthias under 1888/1906; explicit rule `02-29=02-22~02-23o`
    // moves the Vigil to real Feb 22). Letter-'d' Easter years like
    // 2048 don't get an explicit Petri-Damiani transfer rule, so Perl
    // reaches the back-walk fallback.
    let native_is_displaced_vigil = native_entry
        .map(|e| e.main.stem == "02-23o" && month == 2 && day == 24)
        .unwrap_or(false);
    // T1910 over-fire guard: enable heuristic only when no native
    // entry AND we're on the leap-year bissextile shift (real Feb 24
    // = kalendar 02-29) — NOT for plain non-leap dates with empty
    // kalendar entries (e.g. 02-12 in non-1939 layers, where Septem
    // Fundatorum's stem 02-12 lives at kalendar 02-11 under the
    // 1888/1906 layer; transfer-on-preempted-Sunday is NOT what Perl
    // does for those — Perl just renders the feria).
    let t1910_heuristic_eligible = matches!(rubric, Rubric::Tridentine1910)
        && (native_is_displaced_vigil
            || (native_entry.is_none()
                && date::leap_year(year)
                && month == 2
                && day == 24));
    let heuristic_transfer_active =
        matches!(rubric, Rubric::Tridentine1570 | Rubric::Monastic)
        || t1910_heuristic_eligible;
    if heuristic_transfer_active {
    if let Some((stem, name, rank_num)) =
        transferred_sancti_for_1570(year, month, day, layer, corpus)
    {
        let should_apply = match native_entry {
            None => true,
            Some(_) => rank_num > native_rank,
        };
        if should_apply {
            let key = resolve_sancti_stem(&stem, corpus);
            let metadata_key = effective_tempora_key(&key, corpus);
            let mass = corpus.mass_file(&metadata_key);
            let commune = mass
                .and_then(|m| pick_commune_for_rubric(m, rubric))
                .unwrap_or_default();
            let rank_class = mass
                .and_then(|m| pick_rank_class_for_rubric(m, rubric))
                .unwrap_or_default();
            let entry = SanctiEntry {
                rubric: "1570-transferred".into(),
                name,
                rank_class,
                rank_num: Some(rank_num),
                commune,
            };
            return (key, Some(entry));
        }
    }
    }
    let kalendar_lookup = if suppressed_by_transfer {
        None
    } else {
        kalendar_entry_for_date
    };
    if let Some(override_) = kalendar_lookup {
        // Some kalendar entries assume a specific weekday — e.g.
        // 01-12 → 01-12t = "Dominica infra Octavam Epi" assumes
        // Jan 12 is Sunday, which fails in years like 2026 where
        // Jan 12 is Monday. When the override stem points at a
        // file whose officium starts with "Dominica" and the
        // actual date is NOT a Sunday this year, fall back to the
        // bare Sancti/<MM-DD> file (the numerical-day variant —
        // e.g. Sancti/01-12 = "Septima die infra Octavam Epi").
        let stem = redirect_dominica_to_numerical(
            &override_.main.stem,
            year, month, day,
            corpus,
        );
        let key = resolve_sancti_stem(&stem, corpus);
        // Many `t`/`o`-suffixed Sancti stems are body-less redirects
        // (`Sancti/05-09t` = `@Sancti/05-09`); chase the parent chain
        // to find the file that actually carries [Rank]. The winner
        // file_key stays the original (`Sancti/05-09t`) so the
        // resolver still applies the right name-substitution etc.
        let metadata_key = effective_tempora_key(&key, corpus);
        let mass = corpus.mass_file(&metadata_key);
        // Pick the right rank for 1570:
        //   1. `rank_num_1570` from corpus when annotated `(sed rubrica
        //      1570)` (Bibiana 12-02: default 2.2 Semiduplex, 1570 1.1
        //      Simplex).
        //   2. Else the corpus's bare `rank_num` (Christmas 12-25 has
        //      no annotation; bare 6.5 applies under all rubrics —
        //      including 1570).
        //   3. Else the kalendar 1570.txt rank — coarse but at least
        //      tells us whether the date carries a feast.
        // The kalendar's rank is intentionally a *last* resort because
        // it uses an integer 1..7 grading whereas the corpus carries
        // fractional ranks (e.g. Christmas 6.5).
        // 1570-specific rank/commune fields take precedence ONLY when
        // the active layer is the 1570 baseline. For Tridentine 1910
        // and later layers the `(sed rubrica 1570)` annotated values
        // are the wrong choice — they represent the pre-1570 reading
        // for an already-post-1570 universe.
        //
        // Mirrors the upstream `(rubrica 196 aut rubrica 1955)` second
        // [Rank] header on Sancti files like 01-07, 01-12, 03-19,
        // 06-23 — under R55/R60 these become a Feria of class IV with
        // a different commune (`vide Sancti/01-06` instead of `ex
        // Sancti/01-06`). Without this gate the 1955+ rank/commune was
        // being lost and the saint kept its DA-era 5.6 Semiduplex.
        let prefer_1570_overrides =
            matches!(layer, crate::kalendaria_layers::Layer::Pius1570);
        let prefer_t1910 = matches!(rubric, Rubric::Tridentine1910);
        let prefer_r55 = matches!(rubric, Rubric::Reduced1955);
        let prefer_r60 = matches!(rubric, Rubric::Rubrics1960);
        let mut rank_num = mass
            .and_then(|m| {
                if prefer_r60 {
                    // 1960-only first, then 1955+ shared, then default.
                    m.rank_num_1960.or(m.rank_num_1955).or(m.rank_num).or(m.rank_num_1570)
                } else if prefer_r55 {
                    // R55 must NOT pick up an `(rubrica 196)`-only
                    // variant — Patrick (03-17) declares Duplex 2 only
                    // for /196/, so under R55 the default Duplex 3
                    // applies.
                    m.rank_num_1955.or(m.rank_num).or(m.rank_num_1570)
                } else if prefer_t1910 {
                    // T1910 picks up the `(rubrica 1888)` /
                    // `(rubrica 1906)` second-Rank elevation —
                    // Sacred Heart Pent02-5o goes from Duplex majus
                    // (4.01) to Duplex I classis (6.5).
                    m.rank_num_1906.or(m.rank_num)
                } else if prefer_1570_overrides {
                    m.rank_num_1570.or(m.rank_num)
                } else {
                    m.rank_num.or(m.rank_num_1570)
                }
            })
            .unwrap_or(override_.main.rank_num);
        // 1955-only Semiduplex demotion. Pius XII (Cum nostra hac
        // aetate, 1955) abolished the Semiduplex rank — feasts in
        // the 2.2..<2.9 band become Simplex (1.2). Mirrors
        // horascommon.pl:382-390:
        //   if ($version =~ /1955|Monastic.*Divino|1963/
        //       && $srank[2] >= 2.2 && $srank[2] < 2.9
        //       && $srank[1] =~ /Semiduplex/i)
        //   { $srank[2] = 1.2; }
        // R60 (Rubrics 1960) is NOT in that regex — under John XXIII's
        // 1960 reform, Semiduplex feasts merged with Duplex/III
        // classis (kept at 2.2). Without restricting the demotion to
        // R55 only, Saturday-BVM beats III-classis Pope-saints under
        // R60 (Ubaldus 05-16 etc.).
        if prefer_r55 && rank_num >= 2.2 && rank_num < 2.9 {
            let rank_class_str = mass
                .and_then(|m| m.rank.as_deref())
                .unwrap_or_default();
            if rank_class_str.to_ascii_lowercase().contains("semiduplex") {
                rank_num = 1.2;
            }
        }
        let commune = mass
            .and_then(|m| {
                if prefer_1570_overrides {
                    m.commune_1570.clone().or_else(|| m.commune.clone())
                } else {
                    pick_commune_for_rubric(m, rubric)
                }
            })
            .unwrap_or_default();
        let name = mass
            .and_then(|m| {
                if prefer_r60 {
                    m.officium_1960.clone().or_else(|| m.officium_1955.clone())
                } else if prefer_r55 {
                    m.officium_1955.clone()
                } else if prefer_t1910 {
                    m.officium_1906.clone()
                } else {
                    None
                }
            })
            .unwrap_or_else(|| override_.main.name.clone());
        let rank_class = mass
            .and_then(|m| {
                if prefer_r60 {
                    m.rank_1960.clone().or_else(|| m.rank_1955.clone()).or_else(|| m.rank.clone())
                } else if prefer_r55 {
                    m.rank_1955.clone().or_else(|| m.rank.clone())
                } else if prefer_t1910 {
                    m.rank_1906.clone().or_else(|| m.rank.clone())
                } else {
                    m.rank.clone()
                }
            })
            .unwrap_or_default();
        let entry = SanctiEntry {
            rubric: "1570".into(),
            name,
            rank_class,
            rank_num: Some(rank_num),
            commune,
        };
        return (key, Some(entry));
    }
    // No 1570 kalendar entry for this date → date is ferial under
    // Tridentine 1570 (the post-1570 corpus may carry a saint here,
    // but it didn't exist in 1570). Return a placeholder FileKey
    // with no entry; the precedence layer treats `sancti_entry =
    // None` as "no sanctoral office, temporal cycle wins solo".
    let key = FileKey {
        category: FileCategory::Sancti,
        stem: format!("{month:02}-{day:02}"),
    };
    (key, None)
}

/// Parse a commune indication into a typed `(FileKey, CommuneType)`.
///
/// Recognised forms (Tridentine corpus survey, Phase 7):
///
///   `vide C2a-1`           → `Commune/C2a-1` Vide
///   `ex C9`                → `Commune/C9`    Ex
///   `vide Sancti/12-26`    → `Sancti/12-26`  Vide  (Octave-day fallback)
///   `ex Sancti/12-28`      → `Sancti/12-28`  Ex
///   `vide Tempora/Epi1-0a` → `Tempora/Epi1-0a` Vide
///   `vide Epi3-0`          → bare stem; resolved to a `Tempora/`
///                            FileKey by the caller (context-dependent
///                            since `vide Epi3-0` only appears in
///                            Tempora files, but we encode the bare
///                            form here and let the consumer fix the
///                            category if needed).
///
/// `parse_commune_in_context(s, winner_category)` is the variant that
/// knows the winner's category and uses it for bare-stem fallbacks.
#[cfg(test)]
fn parse_commune(s: &str) -> (Option<FileKey>, CommuneType) {
    parse_commune_in_context(s, &FileCategory::Commune)
}

pub fn parse_commune_in_context(
    s: &str,
    winner_category: &FileCategory,
) -> (Option<FileKey>, CommuneType) {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return (None, CommuneType::None);
    }
    let (prefix, rest) = match trimmed.split_once(char::is_whitespace) {
        Some(p) => p,
        None => return (None, CommuneType::None),
    };
    let kind = match prefix.to_ascii_lowercase().as_str() {
        "vide" => CommuneType::Vide,
        "ex" => CommuneType::Ex,
        _ => return (None, CommuneType::None),
    };
    let raw_target = rest.split_whitespace().next().unwrap_or("");
    if raw_target.is_empty() {
        return (None, kind);
    }
    let key = parse_commune_target(raw_target, winner_category);
    (Some(key), kind)
}

fn parse_commune_target(target: &str, winner_category: &FileCategory) -> FileKey {
    if let Some((cat, stem)) = target.split_once('/') {
        // Explicit `Sancti/12-26`, `Tempora/Epi1-0a`, `Commune/C2a-1`.
        // Match case-insensitively: corpus is inconsistent ("ex
        // sancti/08-15" lowercase on Sancti/08-19bmv, "ex Sancti/08-15"
        // uppercase on Sancti/08-20bmv).
        let category = match cat.to_ascii_lowercase().as_str() {
            "sancti" => FileCategory::Sancti,
            "tempora" => FileCategory::Tempora,
            "commune" => FileCategory::Commune,
            "sanctim" => FileCategory::SanctiM,
            "sanctiop" => FileCategory::SanctiOP,
            "sancticist" => FileCategory::SanctiCist,
            _ => FileCategory::Other(cat.to_string()),
        };
        return FileKey {
            category,
            stem: stem.to_string(),
        };
    }
    // Bare stem.  Distinguish by shape:
    //   `Cxx`/`Cxx-y` → Commune/<stem>
    //   anything else → same category as the winner (Tempora seasons,
    //                   when a Tempora file's [Rank] commune column
    //                   says e.g. `vide Epi3-0`).
    let starts_with_c_then_alnum = target
        .strip_prefix('C')
        .map(|rest| rest.chars().next().map_or(false, |c| c.is_ascii_digit()))
        .unwrap_or(false);
    if starts_with_c_then_alnum {
        return FileKey {
            category: FileCategory::Commune,
            stem: target.to_string(),
        };
    }
    FileKey {
        category: winner_category.clone(),
        stem: target.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Date, Locale};
    use crate::corpus::BundledCorpus;

    fn input(year: i32, month: u32, day: u32) -> OfficeInput {
        OfficeInput {
            date: Date::new(year, month, day),
            rubric: Rubric::Tridentine1570,
            locale: Locale::Latin,
            is_mass_context: true,
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
    fn sept_ember_saturday_uses_093_6_not_sat_bvm() {
        // 2026-09-19 = Sat. Pent16-6 (rank 1.0 Feria) → monthday
        // overlay → Tempora/093-6 (rank 2.1 Feria major). Without
        // overlay, Saturday-BVM (rank 1.3) would fire and the propers
        // would be Commune/C10. With overlay, 2.1 ≥ 1.4 → Sat-BVM
        // skipped, winner is Sept Embertide Saturday.
        let r = run(2026, 9, 19);
        assert_eq!(winner_path(&r), "Tempora/093-6");
        assert!(!r.sanctoral_office);
    }

    #[test]
    fn sept_ember_friday_uses_093_5() {
        // 2026-09-18 = Fri. Pent16-5 (rank 1.0) → monthday → 093-5
        // (rank 2.1).
        let r = run(2026, 9, 18);
        assert_eq!(winner_path(&r), "Tempora/093-5");
    }

    #[test]
    fn pent22_dominica_minor_beats_chrysanthus_simplex() {
        // 2026-10-25 = Sun. Pent22-0 = Dominica minor 5.0
        // (downgraded to 2.9 by 1570 rule). Chrysanthus is Simplex
        // 1.1. Sunday wins → winner = Tempora/Pent22-0.
        let r = run(2026, 10, 25);
        assert_eq!(winner_path(&r), "Tempora/Pent22-0");
        assert!(!r.sanctoral_office);
    }

    #[test]
    fn trace_joseph_r60_2000() {
        let mut inp = input(2000, 3, 20);
        inp.rubric = Rubric::Rubrics1960;
        let r = compute_occurrence(&inp, &BundledCorpus);
        eprintln!("2000-03-20 R60 winner={} sanc={} srank={} trank={}",
            r.winner.render(), r.sanctoral_office, r.sanctoral_rank, r.temporal_rank);
        let entries = crate::transfer_table::transfers_for(2000, "1960", 3, 20);
        eprintln!("transfers_for: {:?}", entries);
        let layer = inp.rubric.kalendar_layer();
        let (skey, sentry) = resolve_sancti_for_tridentine_1570(
            inp.date.year, inp.date.month, inp.date.day, layer, inp.rubric, &BundledCorpus,
        );
        eprintln!("resolved: key={} entry={:?}",
            skey.render(), sentry.as_ref().map(|e| (&e.name, e.rank_num)));
    }

    #[test]
    fn imm_conc_outranks_adv2_sunday_r60() {
        // RG 15: Immaculate Conception (12-08) on Adv2 Sunday — under
        // R60 the saint outranks the Sunday in occurrence.
        let mut inp = input(1985, 12, 8);
        inp.rubric = Rubric::Rubrics1960;
        let r = compute_occurrence(&inp, &BundledCorpus);
        assert_eq!(winner_path(&r), "Sancti/12-08");
        assert!(r.sanctoral_office);
    }

    #[test]
    fn conception_bmv_outranks_advent_feria_1570() {
        // 2026-12-08 = Tue, Adv2-2 ferial. The post-1854 "Immaculata
        // Conceptio" doesn't exist in 1570; the 1570 kalendar entry
        // is "Conceptio Beatae Mariae Virginis" rank 3 (Duplex),
        // file `Sancti/12-08o`. It still outranks the Advent feria.
        let r = run(2026, 12, 8);
        assert_eq!(winner_path(&r), "Sancti/12-08o");
        assert!(r.sanctoral_office);
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
    fn commune_parses_vide_sancti_octave() {
        // 01-02 Octave of St Stephen redirects to Sancti/12-26 for
        // every section the Octave file doesn't carry in-line.
        let (k, t) = parse_commune("vide Sancti/12-26");
        assert_eq!(t, CommuneType::Vide);
        let k = k.unwrap();
        assert!(matches!(k.category, FileCategory::Sancti));
        assert_eq!(k.stem, "12-26");
    }

    #[test]
    fn commune_parses_ex_sancti() {
        // 01-04 Octave of Holy Innocents.
        let (k, t) = parse_commune("ex Sancti/12-28");
        assert_eq!(t, CommuneType::Ex);
        let k = k.unwrap();
        assert!(matches!(k.category, FileCategory::Sancti));
        assert_eq!(k.stem, "12-28");
    }

    #[test]
    fn commune_parses_vide_tempora() {
        let (k, t) = parse_commune("vide Tempora/Epi1-0a");
        assert_eq!(t, CommuneType::Vide);
        let k = k.unwrap();
        assert!(matches!(k.category, FileCategory::Tempora));
        assert_eq!(k.stem, "Epi1-0a");
    }

    #[test]
    fn commune_bare_stem_uses_winner_category() {
        // Tempora/Epi3-1's [Rank] commune column is `vide Epi3-0`
        // (no prefix); resolves to Tempora/Epi3-0 because the winner
        // is a Tempora file.
        let (k, t) = parse_commune_in_context("vide Epi3-0", &FileCategory::Tempora);
        assert_eq!(t, CommuneType::Vide);
        let k = k.unwrap();
        assert!(matches!(k.category, FileCategory::Tempora));
        assert_eq!(k.stem, "Epi3-0");
    }

    #[test]
    fn commune_bare_c_stem_resolves_to_commune() {
        // Bare `vide C5` maps to Commune/C5 even when winner is
        // Sancti — the upstream convention.
        let (k, t) = parse_commune_in_context("vide C5", &FileCategory::Sancti);
        assert_eq!(t, CommuneType::Vide);
        let k = k.unwrap();
        assert!(matches!(k.category, FileCategory::Commune));
        assert_eq!(k.stem, "C5");
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
        // S. Joseph (March 19) — under Tridentine 1570 the kalendar
        // table maps 03-19 to `Sancti/03-19t` (rank 3 Duplex). On a
        // Lent ferial, the saint still wins.
        let r = run(2026, 3, 19);
        assert_eq!(winner_path(&r), "Sancti/03-19t");
        assert!(r.sanctoral_office);
    }
}
