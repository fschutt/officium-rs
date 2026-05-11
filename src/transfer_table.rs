//! Sunday-letter and Easter-coded transfer tables.
//!
//! Mirrors the upstream `vendor/divinum-officium/web/www/Tabulae/Transfer/`
//! tree (vendored as `data/transfer_combined.txt`). For each year we
//! consult two files:
//!
//!   * `Transfer/<letter>.txt` — Sunday-letter rule (a..g).
//!   * `Transfer/<easter>.txt` — Easter-day rule, e.g. `405.txt` for
//!     Easter on April 5.
//!
//! Each line is `MM-DD = TARGET_STEM[~ALT_STEM[~...]] ;; RUBRIC_LIST`,
//! where the rubric list is whitespace-separated tokens (`1570`,
//! `1888`, `DA`, `M1617`, …) the entry applies to. We currently
//! filter to the `1570` rubric only — Phase 8 will broaden this.
//!
//! ## Sunday letter for the year
//!
//! Per the Perl `Directorium::load_transfers`:
//! ```text
//! easter = month*100 + day        // e.g. 5 Apr -> 405
//! letter_idx = (easter - 319 + (month==4 ? 1 : 0)) % 7
//! letter     = "abcdefg"[letter_idx]
//! ```

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::date::{geteaster, leap_year};

/// One transfer instruction for a date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferTarget {
    /// Stem to swap in. Leading `Tempora/` is preserved when present
    /// (e.g. `Tempora/Epi4-0tt`); a bare stem (`11-29`) implies
    /// `Sancti/11-29`.
    pub main: String,
    /// Optional `~`-separated commemorations or alt forms.
    pub extras: Vec<String>,
}

/// One file's worth of parsed entries (keyed by `MM-DD`).
type FileEntries = BTreeMap<String, Vec<(TransferTarget, Vec<String>)>>;

/// Whole-corpus index: `file_name` -> per-date entries.
type Combined = BTreeMap<String, FileEntries>;

static TRANSFER_DATA: &str = include_str!("../data/transfer_combined.txt");

fn parsed() -> &'static Combined {
    static PARSED: OnceLock<Combined> = OnceLock::new();
    PARSED.get_or_init(|| {
        let mut out: Combined = BTreeMap::new();
        let mut current_file: Option<String> = None;
        for line in TRANSFER_DATA.lines() {
            if let Some(rest) = line.strip_prefix("# FILE: ") {
                current_file = Some(rest.trim().to_string());
                continue;
            }
            let Some(file) = current_file.as_ref() else {
                continue;
            };
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Split into LHS=RHS;;RUBRICS.
            let (lhs, after_eq) = match trimmed.split_once('=') {
                Some(p) => p,
                None => continue,
            };
            let (rhs, rubrics) = match after_eq.split_once(";;") {
                Some(p) => p,
                None => (after_eq, ""),
            };
            let mm_dd = lhs.trim().to_string();
            let rubric_list: Vec<String> = rubrics
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            let parts: Vec<&str> = rhs.trim().split('~').collect();
            let main = parts[0].trim().to_string();
            let extras: Vec<String> =
                parts[1..].iter().map(|s| s.trim().to_string()).collect();
            let target = TransferTarget { main, extras };
            out.entry(file.clone())
                .or_default()
                .entry(mm_dd)
                .or_default()
                .push((target, rubric_list));
        }
        out
    })
}

/// Sunday-letter for a year (`'a'..='g'`). Mirrors
/// `Directorium.pm:130-134`.
pub fn sunday_letter(year: i32) -> char {
    let (day, month, _) = geteaster(year);
    let easter = (month as i32) * 100 + (day as i32);
    let plus = if month == 4 { 1 } else { 0 };
    let idx = (easter - 319 + plus).rem_euclid(7);
    let letters = ['a', 'b', 'c', 'd', 'e', 'f', 'g'];
    letters[idx as usize]
}

/// Easter-coded transfer file name for a year (e.g. `"405"` when
/// Easter is April 5). Range is `322..=331` (March) and `401..=426`
/// (April).
pub fn easter_code(year: i32) -> u32 {
    let (day, month, _) = geteaster(year);
    month * 100 + day
}

/// Look up transfers applicable to a given (year, rubric, mm-dd).
/// Returns the list of targets (main + extras) from the Sunday-letter
/// file and the Easter-coded file, plus their leap-year companions
/// when applicable.
///
/// In a leap year, the dominical letter shifts at the bissextile day:
/// dates from Jan 1 to Feb 23 (and the inserted 02-29) follow the
/// PRE-leap-day letter (one ahead of the post-leap-day letter), and
/// dates from Feb 24 (which kalendar-shifts to 02-29 → Vigil of
/// Matthias) onward follow the post-leap-day letter. Mirrors
/// `Directorium::load_transfers` lines 132-149.
///
/// Mirrors Perl `Directorium::load_transfer_file` line-filter logic
/// for leap years (`filter == 1`): rules from the bare-letter and
/// bare-easter files whose LHS matches the early-Feb regex OR whose
/// RHS starts with `02-2[0123]` are EXCLUDED. This is what makes
/// `02-25=02-23r;;1888 1906` (letter e) NOT fire on real Feb 25 in
/// leap year — both e.txt's filter-1 (excludes-when-RHS=02-23r) and
/// f.txt's filter-2 (only Jan + Feb 23) drop it. Without the
/// filter, real Feb 25 wrongly resolves to St. Peter Damian
/// (02-23r) instead of St. Matthias on the bissextile-shift date.
pub fn transfers_for(
    year: i32,
    rubric: &str,
    month: u32,
    day: u32,
) -> Vec<TransferTarget> {
    let mm_dd = format!("{month:02}-{day:02}");
    let mut out = Vec::new();
    let parsed = parsed();
    let leap = leap_year(year);
    // Perl filter 1: leap year + bare-letter / bare-easter file +
    // post-leap-day date. Skip rules with LHS in the early-Feb
    // regex OR RHS starting with `02-2[0123]`. We apply the
    // exclusion at lookup time rather than re-parsing the files
    // per (year, date), which keeps the OnceLock cache hot.
    let apply_filter1_exclusion = leap && is_post_leap_day(month, day);
    let files_to_consult = transfer_files_for(year, month, day);
    for fname in files_to_consult {
        let Some(entries) = parsed.get(&fname) else {
            continue;
        };
        let Some(targets) = entries.get(&mm_dd) else {
            continue;
        };
        for (target, rubrics) in targets {
            if !rubric_matches(rubrics, rubric) {
                continue;
            }
            if apply_filter1_exclusion
                && entry_is_early_feb_relevant(&mm_dd, target)
            {
                continue;
            }
            out.push(target.clone());
        }
    }
    out
}

/// Real date is Feb 24..28 (the leap-shifted range that follows the
/// bissextile day). Mirrors the inverse of `is_pre_leap_day` for
/// the filter-1 application range.
fn is_post_leap_day(month: u32, day: u32) -> bool {
    month == 2 && (24..=28).contains(&day)
}

/// Mirrors Perl's `regex2` from `Directorium::load_transfer_file`:
/// `^(?:Hy|seant)?(?:01|02-[01]|02-2[01239]|.*=(01|02-[01]|02-2[0123])|dirge1)`.
/// Returns true when EITHER the LHS (the date key) OR the RHS (the
/// transfer target) implicates an "early Feb" date that the
/// post-leap-day rendering should not see.
fn entry_is_early_feb_relevant(lhs_mm_dd: &str, target: &TransferTarget) -> bool {
    if matches_early_feb(lhs_mm_dd) {
        return true;
    }
    // Strip a leading `Tempora/` or `Sancti/` prefix on the RHS
    // before checking — both forms appear in the parsed targets.
    let rhs = target
        .main
        .strip_prefix("Tempora/")
        .or_else(|| target.main.strip_prefix("Sancti/"))
        .unwrap_or(&target.main);
    if rhs_matches_early_feb_narrow(rhs) {
        return true;
    }
    for extra in &target.extras {
        let extra = extra
            .strip_prefix("Tempora/")
            .or_else(|| extra.strip_prefix("Sancti/"))
            .unwrap_or(extra.as_str());
        if rhs_matches_early_feb_narrow(extra) {
            return true;
        }
    }
    false
}

/// `^(?:01|02-[01]|02-2[01239]|dirge1)` — early-Feb ranges that
/// filter 1 excludes.
fn matches_early_feb(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix("01") {
        return rest.is_empty() || rest.starts_with('-');
    }
    if let Some(rest) = s.strip_prefix("02-") {
        // 02-0X or 02-1X
        if let Some(c) = rest.chars().next() {
            if matches!(c, '0' | '1') {
                return true;
            }
        }
        // 02-20, 02-21, 02-22, 02-23, 02-29
        for prefix in ["20", "21", "22", "23", "29"] {
            if rest.starts_with(prefix) {
                return true;
            }
        }
    }
    s.starts_with("dirge1")
}

/// RHS narrower regex `02-2[0123]` — only 02-20, 02-21, 02-22, 02-23.
/// (Not 02-29; the bis day has its own bookkeeping.)
fn rhs_matches_early_feb_narrow(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix("01") {
        return rest.is_empty() || rest.starts_with('-');
    }
    if let Some(rest) = s.strip_prefix("02-") {
        if let Some(c) = rest.chars().next() {
            if matches!(c, '0' | '1') {
                return true;
            }
        }
        for prefix in ["20", "21", "22", "23"] {
            if rest.starts_with(prefix) {
                return true;
            }
        }
    }
    false
}

/// Picks the right transfer files for `(year, month, day)`. In a
/// leap year, dates from Jan 1 to Feb 23 (and the inserted 02-29)
/// follow the *next* dominical letter and the *next* Easter code
/// (`letter+1` mod 7, `easter_code + 1`). Other dates use the bare
/// year's letter+easter.
fn transfer_files_for(year: i32, month: u32, day: u32) -> Vec<String> {
    let letter = sunday_letter(year);
    let easter = easter_code(year);
    let mut out = Vec::with_capacity(4);
    if leap_year(year) && is_pre_leap_day(month, day) {
        // Use the next letter + next Easter file. Letter advance is
        // cyclic: letters[(idx + 1) % 7]. Perl's `$letters[$letter-6]`
        // exploits Perl negative-index wrap; for `letter='f'` (idx 5)
        // → `idx-6 = -1` → letters[-1] = 'g'. Equivalent to (idx+1)%7.
        let letters = ['a', 'b', 'c', 'd', 'e', 'f', 'g'];
        let idx = letters.iter().position(|&c| c == letter).unwrap_or(0);
        let next_letter = letters[(idx + 1) % 7];
        let next_easter = if easter == 331 { 401 } else { easter + 1 };
        out.push(format!("{}.txt", next_letter));
        out.push(format!("{}.txt", next_easter));
    } else {
        out.push(format!("{}.txt", letter));
        out.push(format!("{}.txt", easter));
    }
    out
}

/// True for dates in `Jan 1 → Feb 23` and `Feb 29` (bissextile day).
/// Mirrors the upstream regex
/// `^(?:Hy|seant)?(?:01|02-[01]|02-2[01239]|dirge1)`. Excludes Feb
/// 24..28 and Mar onward.
fn is_pre_leap_day(month: u32, day: u32) -> bool {
    if month == 1 {
        return true;
    }
    if month == 2 {
        return day < 24 || day == 29;
    }
    false
}

/// True if `rubric` (e.g. `"1570"`, `"DA"`) appears in `rubric_list`.
/// Empty list means "applies always".
fn rubric_matches(rubric_list: &[String], rubric: &str) -> bool {
    if rubric_list.is_empty() {
        return true;
    }
    rubric_list.iter().any(|r| r == rubric)
}

/// True when this `(year, rubric, month, day)`'s native stem is being
/// *transferred away* — i.e. the year's transfer table contains a
/// `xx-yy=mm-dd` rule pointing TO this date's stem (with `xx-yy` ≠
/// the current date). Used to suppress the saint on its native date
/// when it has been moved (typically Annunciation in Holy Week:
/// `04-08=03-25` means "April 8 receives Annunciation; March 25 is
/// vacated").
pub fn stem_transferred_away(
    year: i32,
    rubric: &str,
    month: u32,
    day: u32,
) -> bool {
    stem_transferred_away_with_stems(year, rubric, month, day, &[])
}

/// True when an arbitrary Tempora *stem* has been transferred to a
/// date OTHER than today by the year's transfer table. Used to
/// suppress a stem on its native calendar position when a transfer
/// rule has moved it elsewhere.
///
/// Concretely: under DA, `01-12=Tempora/Epi1-0` says Holy Family
/// (Sunday-after-Epiphany) lives on Jan 12 this year. So if today
/// is Jan 13 (the calendar's default Sunday-after-Epiphany position
/// when 01-13 is Sunday), Holy Family is NOT here — the temporal
/// slot is vacated and Sancti/01-13 (Octave of Epiphany) wins.
///
/// We DON'T scope this to a single LHS month-day — the caller
/// passes today's `(month, day)`; we walk every entry of the
/// year's transfer files and ask "does any rule send `Tempora/<stem>`
/// somewhere ≠ today?".
pub fn temporal_stem_moved_elsewhere(
    year: i32,
    rubric: &str,
    month: u32,
    day: u32,
    stem: &str,
) -> bool {
    let parsed = parsed();
    let today_mm_dd = format!("{month:02}-{day:02}");
    let target_path = format!("Tempora/{stem}");
    for fname in transfer_files_for(year, month, day) {
        let Some(entries) = parsed.get(&fname) else {
            continue;
        };
        for (lhs_mm_dd, targets) in entries {
            if lhs_mm_dd == &today_mm_dd {
                continue; // a rule keyed at today is "place HERE", not "move away"
            }
            for (target, rubrics) in targets {
                if !rubric_matches(rubrics, rubric) {
                    continue;
                }
                if target.main == target_path {
                    return true;
                }
            }
        }
    }
    false
}

/// Like [`stem_transferred_away`], but also checks whether any of
/// the supplied saint stems (e.g. `"02-23o"`, `"04-08o"`) appear in
/// a transfer rule's `extras` list. Mirrors Perl
/// `Directorium::transfered`: the upstream function asks "is this
/// stem mentioned anywhere in the year's transfer table?", not "is
/// this DATE the target?". Both checks are needed to catch the full
/// upstream behaviour:
///
/// - Date-target match: `04-12=04-11;;1570 M1617` (c.txt) — St. Leo's
///   stem 04-11 appears as a target main, suppressing him on 04-11.
/// - Stem-extras match: `02-23=02-22~02-23o;;1570 M1617` (d.txt) —
///   Vigil stem `02-23o` lives in the extras of the 02-23 rule, and
///   Perl uses it to suppress the Vigil on real Feb 24 leap (kalendar
///   02-29) where the bissextile shift would otherwise re-fire it.
pub fn stem_transferred_away_with_stems(
    year: i32,
    rubric: &str,
    month: u32,
    day: u32,
    stems: &[&str],
) -> bool {
    let mm_dd = format!("{month:02}-{day:02}");
    let parsed = parsed();
    let files_to_consult = transfer_files_for(year, month, day);
    for fname in files_to_consult {
        let Some(entries) = parsed.get(&fname) else {
            continue;
        };
        for (source_mmdd, targets) in entries {
            if source_mmdd == &mm_dd {
                continue;
            }
            // Perl `Directorium::transfered()` line 247 / 262:
            //   `next if $key =~ /(dirge|Hy)/i;`
            // Pseudo-keys like `dirge1=02-12` (placement of Office of
            // the Dead) and `Hy*=...` (hymn-shift markers) are not
            // saint transfers — they shouldn't trigger native-saint
            // suppression on the target date. Without this skip, T1910
            // 02-12 (Septem Fundatorum) was being suppressed every
            // year because `dirge1=02-12;;1906` mentions stem 02-12 in
            // its target. Closes Quadp_Quad_Commune_C4a 02-11 days.
            if source_mmdd.eq_ignore_ascii_case("dirge1")
                || source_mmdd.starts_with("dirge")
                || source_mmdd.starts_with("Hy")
                || source_mmdd.starts_with("hy")
            {
                continue;
            }
            for (target, rubrics) in targets {
                if !rubric_matches(rubrics, rubric) {
                    continue;
                }
                // Mirror Perl `transfered()`'s `val !~ /^$key/` guard:
                // a transfer only counts as "moved away" when the
                // rule's target does NOT start with its own source
                // key. `02-23=02-22~02-23o` (key 02-23, target prefix
                // 02-22) → moved. `02-22=02-22~02-23o` (key 02-22,
                // target prefix 02-22 = key) → not moved (placement,
                // not transfer).
                if target.main.starts_with(source_mmdd.as_str()) {
                    continue;
                }
                // Two ways the rule mentions THIS date or stem:
                //   (a) date-target match — `04-12=04-11;;1570` puts
                //       the saint of 04-12 on 04-11; on 04-11 the
                //       native saint (St. Leo, stem 04-11) is
                //       suppressed because target.main == mm_dd.
                //       Only valid when one of the candidate stems
                //       *of this date* is mentioned in the rule's
                //       val (or when no stems were supplied — the
                //       legacy date-only check).
                //   (b) stem-extras match — `02-23=02-22~02-23o`
                //       moves stem 02-23o from 02-23 to 02-22; any
                //       date where the kalendar serves stem 02-23o
                //       (e.g. real Feb 24 leap = kalendar 02-29)
                //       sees this stem in extras and suppresses.
                let target_matches_date = target.main == mm_dd;
                let mentions_a_candidate_stem = stems.iter().any(|stem| {
                    target.main.eq_ignore_ascii_case(stem)
                        || target.extras.iter().any(|e| e.eq_ignore_ascii_case(stem))
                });
                if target_matches_date {
                    if stems.is_empty() || mentions_a_candidate_stem {
                        return true;
                    }
                } else if mentions_a_candidate_stem {
                    return true;
                }
                // Perl `Directorium::transfered()` (Directorium.pm:251)
                // uses regex SUBSTRING matching: `$val =~ /$str/i` OR
                // `$str =~ /$val/i`. The exact-equality checks above
                // miss the suffix case where a rule's extras list
                // includes a stem that CONTAINS our candidate as a
                // prefix — e.g. `01-28=01-27~01-28t;;1570 M1617 1888
                // 1906` (Stransfer/331.txt). Under T1910 (1906 layer)
                // on letter-f easter-331 years, kalendar 01-31 has
                // Petri Nolasci (file stem 01-28); the rule's val
                // `01-27~01-28t` mentions `01-28` only as a substring
                // of `01-28t`, so exact match misses but Perl's
                // substring regex fires → Petri suppressed on his
                // native 01-31. Same shape applies to other letter-f
                // 331 years (1991, 2002, …). Closes the T1910
                // 01-31 Septuagesima-Thursday residual.
                // Narrow Perl-substring-match path: mirrors
                // `$val =~ /$str/i` from `Directorium::transfered`,
                // gated on `source_mmdd == stem`. Triggers when a
                // rule keyed at our stem has its val MENTIONING our
                // stem as a substring (typically because a suffixed
                // sibling like `01-28t` literally contains `01-28`
                // as a prefix).
                //
                // Closes T1910 letter-f easter-331 years (1991, 2002,
                // …): rule `01-28=01-27~01-28t;;1570 M1617 1888 1906`
                // — kalendar 01-31 has Petri Nolasci (file 01-28),
                // and Perl's substring match suppresses him on his
                // native day. The exact-equality checks above miss
                // because no rule's main or extras ever literally
                // equals `01-28` here.
                //
                // Gated on `source_mmdd == stem` so the substring
                // match doesn't bleed into unrelated rules: a rule
                // like `02-29=02-22~02-23o` (source 02-29) shouldn't
                // suppress stem 02-23 just because the extras
                // happen to contain a `02-23`-prefixed string. The
                // 1976 letter-c leap-year case (d.txt `01-28=01-18`)
                // also stays quiet — its val "01-18" doesn't mention
                // "01-28".
                let key_matches_a_stem = stems.iter().any(|s| {
                    source_mmdd.eq_ignore_ascii_case(s)
                });
                let val_substring_mentions_stem = stems.iter().any(|stem| {
                    let s: &str = stem;
                    target.main.contains(s)
                        || target.extras.iter().any(|e| e.contains(s))
                });
                if key_matches_a_stem && val_substring_mentions_stem {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sunday_letter_2026_is_d() {
        // 2026 Easter = April 5. (4*100 + 5 - 319 + 1) % 7 = 87 % 7 = 3 → 'd'.
        assert_eq!(sunday_letter(2026), 'd');
    }

    #[test]
    fn easter_code_2026_is_405() {
        assert_eq!(easter_code(2026), 405);
    }

    #[test]
    fn andrew_vigil_transfers_to_saturday_2026() {
        // d.txt: `11-28=11-29;;1570 1888 1906 DA M1617 M1930`.
        // For 1570 in 2026 → Vigil of Andrew (11-29) is read on Nov 28.
        let t = transfers_for(2026, "1570", 11, 28);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].main, "11-29");
    }

    #[test]
    fn epi4_anticipata_jan_31_2026() {
        // 405.txt: `01-31=Tempora/Epi4-0tt;;1570 ...`.
        let t = transfers_for(2026, "1570", 1, 31);
        assert!(!t.is_empty());
        assert_eq!(t[0].main, "Tempora/Epi4-0tt");
    }

    #[test]
    fn no_transfer_for_random_date() {
        let t = transfers_for(2026, "1570", 6, 15);
        assert!(t.is_empty());
    }

    #[test]
    fn leap_year_uses_next_letter_for_jan_feb23() {
        // 2024 (leap): Easter = March 31, post-leap Sunday letter = 'f',
        // pre-leap-day letter = 'g'. The file `g.txt` has the entry
        // `01-19=01-14~01-19;;1570` (Hilarius transferred to Jan 19);
        // without the leap-year shift this rule wouldn't fire because
        // the regular letter file (f.txt) doesn't include it.
        let t = transfers_for(2024, "1570", 1, 19);
        assert!(!t.is_empty(), "expected Hilarius transfer to fire in 2024");
        assert_eq!(t[0].main, "01-14");
    }

    #[test]
    fn leap_year_post_leap_dates_use_post_leap_letter() {
        // 2024 March 25 is Holy Monday (Easter March 31 = letter f);
        // f.txt's transfer entry `04-08=03-25;;` (no rubric tag = all)
        // → Annunciation moved to April 8 in 2024.
        let t = transfers_for(2024, "1570", 4, 8);
        assert!(!t.is_empty(), "expected Annunciation transfer to April 8");
        assert_eq!(t[0].main, "03-25");
    }

    #[test]
    fn stem_transferred_away_fires_on_03_25_when_easter_march_31() {
        // 2024: 03-25 native Annunciation is moved to 04-08 — the
        // native date should report itself as transferred away.
        assert!(stem_transferred_away(2024, "1570", 3, 25));
    }

    #[test]
    fn stem_transferred_away_quiet_for_unaffected_date() {
        // 2024 June 15: no transfer entries point to or from this date.
        assert!(!stem_transferred_away(2024, "1570", 6, 15));
    }

    #[test]
    fn stem_transferred_away_quiet_in_year_without_clash() {
        // 2025: Easter April 20, no Annunciation transfer needed —
        // the 03-25 stem stays on its native date.
        assert!(!stem_transferred_away(2025, "1570", 3, 25));
    }

    #[test]
    fn probe_carmel_to_sab_1977_r60() {
        // 1977 is letter b (Easter Apr 10). b.txt:
        //   `07-16=07-16sab;;1960 Newcal`
        // Under R60 (transfer tag "1960"), Sat 07-16-1977 office =
        // Sancti/07-16sab — the BVM-Sabbato variant that inherits
        // Carmel's [Oratio] via @Sancti/07-16.
        let t = transfers_for(1977, "1960", 7, 16);
        eprintln!("transfers_for(1977, \"1960\", 7, 16) = {:?}", t);
        assert!(!t.is_empty());
        assert_eq!(t[0].main, "07-16sab");
    }

    #[test]
    fn all_souls_to_monday_1980_r60() {
        // 1980 letter e (Easter Apr 6). e.txt has:
        //   `11-03=11-03sec;;1955 1960 M1963 M1963B`
        // Under R60 (transfer tag "1960"), Mon 11-03-1980 = deferred
        // All Souls (Sancti/11-03sec @-inherits from Sancti/11-02).
        let t = transfers_for(1980, "1960", 11, 3);
        assert!(!t.is_empty(), "expected 11-03=11-03sec under R60 letter e");
        assert_eq!(t[0].main, "11-03sec");
    }
}

#[cfg(test)]
mod transferred_all_souls_office {
    use crate::corpus::BundledCorpus;
    use crate::core::{Date, Locale, OfficeInput, Rubric};
    use crate::precedence::compute_office;

    #[test]
    fn probe_07_16sab_rank() {
        // Active rank for Sancti/07-16sab under R60 should be 1.4
        // (horas-side [Rank] "Sanctæ Mariæ Sabbato;;Feria;;1.4;;ex C10").
        let r = crate::horas::active_rank_line_with_annotations(
            "Sancti/07-16sab",
            Rubric::Rubrics1960,
            "",
        );
        eprintln!("07-16sab R60 active_rank = {:?}", r);
    }

    #[test]
    fn probe_07_16_1977_r60_office_winner() {
        let input = OfficeInput {
            date: Date::new(1977, 7, 16),
            rubric: Rubric::Rubrics1960,
            locale: Locale::Latin,
            is_mass_context: false,
        };
        let office = compute_office(&input, &BundledCorpus);
        eprintln!("07-16-1977 R60 winner = {:?}", office.winner.render());
    }

    #[test]
    fn mon_11_03_1980_r60_winner_is_transferred_all_souls() {
        // 1980 is letter e (Easter Apr 6). e.txt:
        //   `11-03=11-03sec;;1955 1960 M1963 M1963B`
        // So Mon 11-03-1980 under R60 (transfer tag "1960") becomes
        // the deferred All Souls (Sancti/11-03sec @-inherits from
        // Sancti/11-02). Documents the upstream-mirror flow used by
        // slice 135's horas.rs Prima / Compline splice extension.
        let input = OfficeInput {
            date: Date::new(1980, 11, 3),
            rubric: Rubric::Rubrics1960,
            locale: Locale::Latin,
            is_mass_context: false,
        };
        let office = compute_office(&input, &BundledCorpus);
        assert_eq!(office.winner.render(), "Sancti/11-03sec");
    }
}

#[cfg(test)]
mod sat_11_03_probes {
    use crate::corpus::BundledCorpus;
    use crate::core::{Date, Locale, OfficeInput, Rubric};
    use crate::precedence::compute_office;

    #[test]
    fn probe_sat_11_03_1979_r55_winner() {
        let input = OfficeInput {
            date: Date::new(1979, 11, 3),
            rubric: Rubric::Reduced1955,
            locale: Locale::Latin,
            is_mass_context: false,
        };
        eprintln!("Sat 11-03-1979 R55 = {:?}", compute_office(&input, &BundledCorpus).winner.render());
    }

    #[test]
    fn probe_tue_11_03_2026_r55_winner() {
        let input = OfficeInput {
            date: Date::new(2026, 11, 3),
            rubric: Rubric::Reduced1955,
            locale: Locale::Latin,
            is_mass_context: false,
        };
        eprintln!("Tue 11-03-2026 R55 = {:?}", compute_office(&input, &BundledCorpus).winner.render());
    }
}

#[cfg(test)]
mod holy_name_probes {
    use crate::corpus::BundledCorpus;
    use crate::core::{Date, Locale, OfficeInput, Rubric};
    use crate::precedence::compute_office;

    #[test]
    fn probe_sun_01_14_1979_t1910() {
        let input = OfficeInput {
            date: Date::new(1979, 1, 14),
            rubric: Rubric::Tridentine1910,
            locale: Locale::Latin,
            is_mass_context: false,
        };
        eprintln!("Sun 01-14-1979 T1910 = {:?}", compute_office(&input, &BundledCorpus).winner.render());
    }
    #[test]
    fn probe_sat_01_13_1979_t1910() {
        let input = OfficeInput {
            date: Date::new(1979, 1, 13),
            rubric: Rubric::Tridentine1910,
            locale: Locale::Latin,
            is_mass_context: false,
        };
        eprintln!("Sat 01-13-1979 T1910 = {:?}", compute_office(&input, &BundledCorpus).winner.render());
    }
}

#[cfg(test)]
mod jan_13_probes {
    use crate::core::Rubric;
    #[test]
    fn probe_01_13_rank_title() {
        let r = crate::horas::active_rank_line_with_annotations(
            "Sancti/01-13",
            Rubric::Tridentine1910,
            "Vespera",
        );
        eprintln!("Sancti/01-13 T1910 rank = {:?}", r);
    }
}

#[cfg(test)]
mod matthias_vigil_probes {
    use crate::corpus::BundledCorpus;
    use crate::core::{Date, Locale, OfficeInput, Rubric};
    use crate::precedence::compute_office;
    #[test]
    fn probe_02_24_1982_t1570() {
        let input = OfficeInput {
            date: Date::new(1982, 2, 24),
            rubric: Rubric::Tridentine1570,
            locale: Locale::Latin,
            is_mass_context: false,
        };
        eprintln!("Wed 02-24-1982 T1570 = {:?}", compute_office(&input, &BundledCorpus).winner.render());
    }
    #[test]
    fn probe_02_23_1982_t1570() {
        let input = OfficeInput {
            date: Date::new(1982, 2, 23),
            rubric: Rubric::Tridentine1570,
            locale: Locale::Latin,
            is_mass_context: false,
        };
        eprintln!("Tue 02-23-1982 T1570 = {:?}", compute_office(&input, &BundledCorpus).winner.render());
    }
}
