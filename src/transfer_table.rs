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
pub fn transfers_for(
    year: i32,
    rubric: &str,
    month: u32,
    day: u32,
) -> Vec<TransferTarget> {
    let mm_dd = format!("{month:02}-{day:02}");
    let mut out = Vec::new();
    let parsed = parsed();
    let files_to_consult = transfer_files_for(year, month, day);
    for fname in files_to_consult {
        let Some(entries) = parsed.get(&fname) else {
            continue;
        };
        let Some(targets) = entries.get(&mm_dd) else {
            continue;
        };
        for (target, rubrics) in targets {
            if rubric_matches(rubrics, rubric) {
                out.push(target.clone());
            }
        }
    }
    out
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
            for (target, rubrics) in targets {
                if !rubric_matches(rubrics, rubric) {
                    continue;
                }
                if target.main == mm_dd {
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
}
