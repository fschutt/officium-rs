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
use crate::divinum_officium::kalendarium_1570;
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
    let tempora_stem_default = if is_nat_label(&weekname) {
        weekname.clone()
    } else {
        format!("{}-{}", weekname, dow)
    };
    // Tridentine 1570 prefers `-a` suffix variants for Sunday Tempora
    // files when they exist (e.g., Tempora/Epi1-0a is the 1570
    // "Dominica infra Octavam Epiphaniae", whereas Tempora/Epi1-0 is
    // the post-1911 "Sancta Familia"). The `-tt` and other suffixes
    // map to other rubric layers; we only chase `-a` for 1570.
    let tempora_stem = pick_tempora_variant_for_1570(&tempora_stem_default, corpus);
    let tempora_key = FileKey {
        category: FileCategory::Tempora,
        stem: tempora_stem,
    };
    // For body-less redirect files (Tempora/Adv1-0o is just a single
    // `@Tempora/Adv1-0` parent-inherit line), follow the parent chain
    // to find the file that actually carries the rank/officium.
    let effective_tempora_key = effective_tempora_key(&tempora_key, corpus);
    let tempora_file = corpus.mass_file(&effective_tempora_key);
    let temporal_rank = tempora_file
        .and_then(|f| f.rank_num)
        .map(|r| downgrade_post_1570_octave(r, tempora_file.unwrap()))
        .unwrap_or(0.0);

    // ── Sanctoral side ───────────────────────────────────────────────
    let (sancti_key, sancti_entry_holder) =
        resolve_sancti_for_tridentine_1570(y, m, d, corpus);
    let sancti_entry: Option<&SanctiEntry> = sancti_entry_holder.as_ref();
    let sanctoral_rank = sancti_entry.and_then(|e| e.rank_num).unwrap_or(0.0);

    // ── Precedence ───────────────────────────────────────────────────
    let sanctoral_office = decide_sanctoral_wins_1570(
        sancti_entry,
        tempora_file,
        temporal_rank,
        sanctoral_rank,
    );

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
        let (commune, commune_type) =
            parse_commune_in_context(&sancti.commune, &sancti_key.category);
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
            Some(f) => parse_commune_in_context(
                f.commune.as_deref().unwrap_or(""),
                &tempora_key.category,
            ),
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

    // "Dominica minor" rule (Tridentine 1570): post-Easter and
    // post-Pentecost Sundays of rank 4.3..5.0 are outranked by any
    // Duplex feast. Mirrors `horascommon.pl:422-433`:
    //   if version =~ /Trid/i && ($trank[2] < 5.1 && $trank[2] > 4.2
    //   && $trank[0] =~ /Dominica/i) { $trank[2] = 2.9 }
    // Without this downgrade Inventio Crucis (5.1) wouldn't beat
    // Dominica IV post Pascha (5.0).
    let temporal_name = tempora.officium.as_deref().unwrap_or("");
    let is_dominica = temporal_name.starts_with("Dominica");
    if is_dominica && trank > 4.2 && trank < 5.1 {
        trank = 2.9;
    }
    // Same rule for "infra octavam Corp[oris Christi]".
    if temporal_name.contains("infra octavam Corp")
        && trank > 4.2
        && trank < 5.1
    {
        trank = 2.9;
    }

    // Sunday handling. Detect via the rendered name — pre-1960 Sundays
    // are written as `Dominica …`. (The Perl uses regex on `$trank[0]`
    // and `$dayname[0]`; we approximate with the officium string.)
    let is_sunday = is_dominica && trank >= 5.1;
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

/// Downgrade post-1570 Octave-day Tempora ranks to feria for the
/// 1570 occurrence pipeline. The corpus carries Sacred Heart Octave
/// (post-1856) and Christ the King Octave (post-1925) entries with
/// elevated `rank_num` (e.g. 2.1 Semiduplex). Under 1570 those days
/// were ordinary ferias of the Pentecost cycle. We detect them via
/// officium-string match: `Cordis Jesu` (Sacred Heart) and `Christi
/// Regis` (Christ the King) are the recognised post-1570 octaves.
fn downgrade_post_1570_octave(rank: f32, file: &MassFile) -> f32 {
    let officium = file.officium.as_deref().unwrap_or("");
    // Post-1856 Sacred Heart octave + post-1925 Christ-the-King.
    // Corpus Christi octave existed in 1570 (Tridentine), so no
    // downgrade for `octavam Corporis Christi` — the Friday's
    // Semiduplex II classis rank is correct under 1570 too.
    let has_post_1570_octave = officium.contains("Cordis Jesu")
        || officium.contains("Cordis Iesu")
        || officium.contains("Sacratissimi")
        || officium.contains("Christi Regis")
        // Patrocinii Sancti Joseph (Pius IX, 1856) — added an
        // octave to the Easter cycle. In 1570 these days were
        // regular Easter ferias. Match permissively because upstream
        // is inconsistent about dot/case ("Patrocinii St. Joseph",
        // "Patrocinii St Joseph", "Patrocínii"). See
        // UPSTREAM_WEIRDNESSES.md #4.
        || officium.contains("Patrocinii")
        || officium.contains("Patrocínii");
    if has_post_1570_octave {
        return 1.0; // ordinary feria
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
    } else if weekname.starts_with("Epi") || weekname.starts_with("Quad") {
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

/// Pick the Tridentine-1570 variant of a Tempora stem when one
/// exists. The corpus uses the `-a` suffix to store the 1570 form of
/// Sundays where a *post-1570* feast has since bumped the original
/// Sunday Mass off the calendar:
///
///   * `Epi1-0`   = post-1911 Holy Family            → 1570: `Epi1-0a`
///   * `Pent01-0` = post-1334 Trinity Sunday         → 1570: `Pent01-0`
///                  (Trinity Sunday already existed in 1570; keep
///                   the bare stem.)
///
/// Trinity Sunday is the only `-a` Sunday in the corpus where 1570
/// uses the BARE form, so we encode a tiny explicit allowlist of
/// `-a` chase candidates rather than a blanket "always chase if
/// `-a` exists" rule (which would mis-direct Trinity).
fn pick_tempora_variant_for_1570(stem: &str, corpus: &dyn Corpus) -> String {
    // Authoritative: upstream `Tabulae/Tempora/Generale.txt` filtered
    // to 1570 entries (vendored as `data/tempora_redirects_1570.txt`).
    // Covers Adv*-* → Adv*-*o, Nat29..Nat31 → *o, Epi1-0 → Epi1-0a,
    // Quad{2..5}-* → *t, Pasc*-* → various, Pent*-* → various.
    if let Some(target) = crate::divinum_officium::tempora_table::redirect_1570(stem) {
        let key = FileKey {
            category: FileCategory::Tempora,
            stem: target.to_string(),
        };
        if corpus.mass_file(&key).is_some() {
            return target.to_string();
        }
    }
    // Below: legacy ad-hoc fallbacks. Most overlap with the table now,
    // but kept as a safety net while we audit the table coverage.
    if TRIDENTINE_1570_TEMPORA_A_CHASE.contains(&stem) {
        let candidate = format!("{stem}a");
        let key = FileKey {
            category: FileCategory::Tempora,
            stem: candidate.clone(),
        };
        if corpus.mass_file(&key).is_some() {
            return candidate;
        }
    }
    if TRIDENTINE_1570_TEMPORA_R_CHASE.contains(&stem) {
        let candidate = format!("{stem}r");
        let key = FileKey {
            category: FileCategory::Tempora,
            stem: candidate.clone(),
        };
        if corpus.mass_file(&key).is_some() {
            return candidate;
        }
    }
    let feria_stem = format!("{stem}Feria");
    let feria_key = FileKey {
        category: FileCategory::Tempora,
        stem: feria_stem.clone(),
    };
    if corpus.mass_file(&feria_key).is_some() {
        return feria_stem;
    }
    stem.to_string()
}

/// Tempora stems where the `-a` variant is the correct 1570 form.
/// Audit any addition: Trinity Sunday (`Pent01-0`) is *not* in this
/// list because Trinity already existed in 1570.
const TRIDENTINE_1570_TEMPORA_A_CHASE: &[&str] = &[
    "Epi1-0", // post-1911 Holy Family bumps the 1570 Dominica infra Octavam
];

/// Tempora stems where the `-0r` variant is the correct 1570 form.
/// These are Sundays whose own propers (Dominica III/IV/V/VI post
/// Pentecosten) were preempted by post-1856 octave-day feasts in
/// the corpus's bare `<stem>-0` slot; the `-0r` suffix preserves the
/// 1570 Sunday body.
const TRIDENTINE_1570_TEMPORA_R_CHASE: &[&str] = &[
    "Pent03-0", // bare `Pent03-0` is Sacred Heart Octave Day; -0r is the 1570 Sunday III post Pent
    "Pent04-0", // similar for IV, V, VI as the calendar shifts
    "Pent05-0",
    "Pent06-0",
];

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
        let Some(entry) = kalendarium_1570::lookup(cursor_m, cursor_d) else {
            continue;
        };
        // Octave-day saints ("Septima die infra Octavam ...", etc.)
        // don't transfer in 1570 — they're commemorated or lost when
        // preempted, never moved. Only fixed-date proper saints
        // transfer. Heuristic match on the kalendar's saint-name.
        if is_octave_day_kalendar_name(&entry.main.name) {
            return None;
        }
        // Was this kalendar entry preempted on its native date?
        let was_preempted = was_sancti_preempted_1570(
            cursor_y, cursor_m, cursor_d, entry, corpus,
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
            let native_here = kalendarium_1570::lookup(walk_m, walk_d);
            let blocked = match native_here {
                None => false, // free
                Some(e) => e.main.rank_num >= entry.main.rank_num,
            };
            if blocked {
                continue; // walk further — this day claims its native
            }
            // Slot available. Does the temporal still preempt the
            // transferred saint here?
            if was_sancti_preempted_1570(walk_y, walk_m, walk_d, entry, corpus) {
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
            2 => if crate::divinum_officium::date::leap_year(year) { 29 } else { 28 },
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
        2 => if crate::divinum_officium::date::leap_year(year) { 29 } else { 28 },
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
    corpus: &dyn Corpus,
) -> bool {
    use crate::divinum_officium::date;
    let weekname = date::getweek(day, month, year, false, true);
    let dow = date::day_of_week(day, month, year);
    let tempora_stem_default = if dow == 0 {
        format!("{}-0", weekname)
    } else {
        format!("{}-{}", weekname, dow)
    };
    let tempora_stem = pick_tempora_variant_for_1570(&tempora_stem_default, corpus);
    let tempora_key = FileKey {
        category: FileCategory::Tempora,
        stem: tempora_stem,
    };
    let effective_key = effective_tempora_key(&tempora_key, corpus);
    let tempora_file = corpus.mass_file(&effective_key);
    let mut trank = tempora_file
        .and_then(|f| f.rank_num)
        .map(|r| downgrade_post_1570_octave(r, tempora_file.unwrap()))
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
    trank > entry.main.rank_num
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
        match file.parent.as_deref() {
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
        if let Some(p) = file.parent.as_deref() {
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
    corpus: &dyn Corpus,
) -> (FileKey, Option<SanctiEntry>) {
    // Transferred-feast lookup: scan back up to 6 days for a Sancti
    // that was preempted on its native date by a higher-ranked
    // Sunday/feast. Apply when:
    //   1. Today has no native kalendar entry, OR
    //   2. Today's native entry has lower rank than the transferred
    //      saint (the transferred saint then displaces today's native
    //      saint, who gets commemorated).
    let native_entry = kalendarium_1570::lookup(month, day);
    let native_rank = native_entry.map(|e| e.main.rank_num).unwrap_or(0.0);
    if let Some((stem, name, rank_num)) =
        transferred_sancti_for_1570(year, month, day, corpus)
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
                .and_then(|m| m.commune_1570.clone().or_else(|| m.commune.clone()))
                .unwrap_or_default();
            let entry = SanctiEntry {
                rubric: "1570-transferred".into(),
                name,
                rank_class: mass.and_then(|m| m.rank.clone()).unwrap_or_default(),
                rank_num: Some(rank_num),
                commune,
            };
            return (key, Some(entry));
        }
    }
    if let Some(override_) = kalendarium_1570::lookup(month, day) {
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
        let rank_num = mass
            .and_then(|m| m.rank_num_1570.or(m.rank_num))
            .unwrap_or(override_.main.rank_num);
        let commune = mass
            .and_then(|m| m.commune_1570.clone().or_else(|| m.commune.clone()))
            .unwrap_or_default();
        let entry = SanctiEntry {
            rubric: "1570".into(),
            name: override_.main.name.clone(),
            rank_class: mass.and_then(|m| m.rank.clone()).unwrap_or_default(),
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

fn parse_commune_in_context(
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
