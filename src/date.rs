//! date.rs
//!
//! This module provides date manipulation and liturgical-calendar-specific
//! routines, mirroring the behavior of the original `Date.pm` from the
//! Divinum Officium project. The functions here are a direct translation
//! (with slight Rust adaptations) of the corresponding Perl code, ensuring
//! that the same logic is preserved. All dates are handled using a purely
//! algorithmic approach, avoiding external libraries, and thus can handle
//! historical dates outside the usual platform range if needed.
//!
//! # Overview
//!
//! The module includes:
//!
//! - **`leap_year(year)`**: Checks whether a given year is leap.
//! - **`geteaster(year)`**: Computes the date of Easter for a given year (Gregorian).
//! - **`getadvent(year)`**: Computes the day-of-year for the First Sunday of Advent.
//! - **`day_of_week(day, month, year)`**: Returns the weekday (0 = Sunday, 1 = Monday, …, 6 = Saturday).
//! - **`date_to_ydays(day, month, year)`**: Converts a date to its day-of-year index (1-based).
//! - **`ydays_to_date(day_of_year, year)`**: Converts a day-of-year back into (day, month, year).
//! - **`getweek(day, month, year, tomorrow, missa)`**: Determines the liturgical week label (e.g. "Adv1", "Quad3", etc.).
//! - **`monthday(day, month, year, modernstyle, tomorrow)`**: Helper for the August–December block in the older rubrics.
//! - **`get_sday(month, day, year)`**: Returns the month-day string used by the `Sancti` data (handles leap day logic).
//! - **`nextday(month, day, year)`**: Returns the `Sancti`-style string (`MM-DD`) for the next calendar day (for Vespers usage).
//! - **`prevnext(date_str, inc)`**: Shifts a date string `MM-DD-YYYY` by `inc` days forward/backward, returning a new string.
//! - **`days_to_date(days)`**: Converts a day-count since 1970-01-01 to a date breakdown (similar to localtime-like structure).
//! - **`date_to_days(day, month, year)`**: Converts a date to the count of days since 1970-01-01 (mirroring the Perl logic).
//!
//! Most of these functions exist to support the Divinum Officium rubrical
//! complexities, especially around the older calendar rules (e.g. the “leap
//! day” numbering issue in February). For new usage, prefer a thorough date
//! library, unless compatibility with the project’s original logic is required.

// chrono removed: this crate uses only the non-chrono fallback paths
// so consumers don't pull a date-library dep just for a fast path.

/// Returns `true` if the given year is a leap year under the Gregorian rules.
///
/// ```
/// # use officium_rs::date::leap_year;
/// assert!(leap_year(2000));  // divisible by 400
/// assert!(!leap_year(1900)); // divisible by 100 but not 400
/// assert!(leap_year(2024));  // divisible by 4 but not 100
/// assert!(!leap_year(2023));
/// ```
pub fn leap_year(year: i32) -> bool {
    // A year is leap if:
    // 1) It is divisible by 4, AND
    // 2) It is not divisible by 100, unless it is also divisible by 400.
    (year % 4 == 0) && ((year % 100 != 0) || (year % 400 == 0))
}

/// Computes the date of Easter (day, month, year) for the given year (Gregorian).
///
/// This follows the algorithm also found in `Date::Easter` (CPAN), known as
/// “Anonymous Gregorian Computus”:
///
/// ```
/// # use officium_rs::date::geteaster;
/// let (eday, emonth, eyear) = geteaster(2024);
/// // Easter 2024 is 03-31-2024
/// assert_eq!((eday, emonth, eyear), (31, 3, 2024));
/// ```
pub fn geteaster(year: i32) -> (u32, u32, i32) {
    // G = year mod 19
    // C = year / 100
    // H = (C - C/4 - (8*C+13)/25 + 19*G + 15) mod 30
    // I = H - (H/28)*(1 - (H/28)*(29/(H+1))*( (21 - G)/11 ))
    // J = (year + year/4 + I + 2 - C + C/4) mod 7
    // L = I - J
    // Easter month = 3 + (L+40)/44
    // Easter day   = L + 28 - 31*(Easter month/4)
    let y = year as i64;
    let g = y % 19;
    let c = y / 100;
    let h = (c - c / 4 - (8 * c + 13) / 25 + 19 * g + 15) % 30;
    let i = h
        - (h / 28)
            * (1
                - (h / 28)
                    * ((29 / (h + 1)) * ((21 - g) / 11)));
    let j = (y + y / 4 + i + 2 - c + c / 4) % 7;
    let l = i - j;
    let month = 3 + ((l + 40) / 44) as u32; // int division
    let day = (l + 28 - 31 * (month as i64 / 4)) as u32;
    (day, month, year)
}

/// Returns the day-of-year (1-based) for the First Sunday of Advent of `year`.
///
/// The First Sunday of Advent is the Sunday nearest to November 30 (St. Andrew),
/// but always before December 25. For the Divinum Officium logic, it’s computed
/// by backing up from Christmas to the previous Sunday minus 21 days (3 weeks).
///
/// ```
/// # use officium_rs::date::getadvent;
/// let advent_2023 = getadvent(2023);
/// assert_eq!(getadvent(2023), 337);
/// ```
pub fn getadvent(year: i32) -> u32 {
    let christmas_ydays = date_to_ydays(25, 12, year);
    let christmas_dow = day_of_week(25, 12, year);
    // Perl `Date.pm::getadvent` uses `day_of_week(...) || 7` — the
    // Perl truthy-or trick that converts dow=0 (Christmas-on-Sunday)
    // to 7 so we walk a full extra week back. Without it, years
    // like 2022 (Christmas on Sunday) place Advent-1 on Dec 4
    // instead of Nov 27, off-by-7 across the entire Advent cycle.
    let dow_for_offset = if christmas_dow == 0 { 7 } else { christmas_dow };
    let advent1 = christmas_ydays as i32 - dow_for_offset as i32 - 21;
    advent1 as u32
}

/// Computes the day of week, returning:
/// - 0 = Sunday
/// - 1 = Monday
/// - 2 = Tuesday
/// - 3 = Wednesday
/// - 4 = Thursday
/// - 5 = Friday
/// - 6 = Saturday
///
/// This matches the original Divinum Officium logic by taking:
/// `(year * 365 + floor((year-1)/4) - floor((year-1)/100) + floor((year-1)/400) - 1 + date_to_ydays(day,month,year)) mod 7`
///
/// ```
/// # use officium_rs::date::day_of_week;
/// let wday = day_of_week(25, 12, 2023); // 0=Sunday, 1=Monday, ...
/// // 25 Dec 2023 is a Monday => 1
/// assert_eq!(wday, 1);
/// ```
pub fn day_of_week(day: u32, month: u32, year: i32) -> u32 {
    // Replicates the same arithmetic from the original code.
    // This approach effectively maps the date to an ordinal then mod 7
    // with an offset that ensures 0 => Sunday.
    let y = year as i64;
    let sum_years = y * 365
        + (y - 1) / 4
        - (y - 1) / 100
        + (y - 1) / 400
        - 1;
    let day_of_year = date_to_ydays(day, month, year) as i64;
    let dow = (sum_years + day_of_year) % 7;
    // In Rust, remainder can be negative if sum_years+day_of_year < 0,
    // but in typical usage here (years post 1582) it's safe.
    // For completeness, ensure we return a positive value in [0..6].
    ((dow + 7) % 7) as u32
}

/// Converts a date to its day-of-year index (1-based).
///
/// ```
/// # use officium_rs::date::date_to_ydays;
/// assert_eq!(date_to_ydays(1, 1, 2023), 1);
/// assert_eq!(date_to_ydays(31, 12, 2023), 365);
/// // For a leap year:
/// assert_eq!(date_to_ydays(1, 3, 2024), 61); // Jan(31) + Feb(29) + 1
/// ```
pub fn date_to_ydays(day: u32, month: u32, year: i32) -> u32 {
    // We'll sum the days in the months prior to `month`, plus `day`.
    // 1-based index, so Jan 1 = 1.
    let months_cum = [0_u32, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let mut days = months_cum[(month - 1) as usize] + day;
    if month > 2 && leap_year(year) {
        days += 1;
    }
    days
}

/// Converts a 1-based day-of-year index back into `(day, month, year)`.
///
/// ```
/// # use officium_rs::date::ydays_to_date;
/// let (day, month, year) = ydays_to_date(365, 2023);
/// // 31 Dec 2023
/// assert_eq!((day, month, year), (31, 12, 2023));
/// ```
pub fn ydays_to_date(day_of_year: u32, year: i32) -> (u32, u32, i32) {
    // We'll pick the correct month by iterating. The original code
    // in Perl does a manual approach. We replicate it here.
    let mut months_cum = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if leap_year(year) {
        months_cum[2] = 29;
    }
    let mut m = 1;
    let mut d = day_of_year;
    while m <= 12 && d > months_cum[m as usize] {
        d -= months_cum[m as usize];
        m += 1;
    }
    (d, m, year)
}

/// Returns a liturgical “week label” string, e.g. "Adv1", "Quad3", "Pent05", etc.,
/// used by Divinum Officium. This function is quite specialized:
///
/// - `day`, `month`, `year` = the current date
/// - `tomorrow` indicates if we want the label for the “next” day (used in e.g. Vesper logic)
/// - `missa` toggles a small variant label in the post-Pentecost/Epiphany season
///
/// This function replicates a complicated logic deciding which period
/// of the year we are in (Advent, Christmas, Epiphany, Septuagesima,
/// Lent, Easter, Pentecost, or after Pentecost).
///
/// ```
/// # use officium_rs::date::getweek;
/// let week_label = getweek(20, 12, 2023, false, false);
/// assert_eq!(week_label, "Adv3".to_string()); // third week of advent
/// ```
pub fn getweek(
    day: u32,
    month: u32,
    year: i32,
    tomorrow: bool,
    missa: bool,
) -> String {
    // Convert to day-of-year, possibly increment for "tomorrow."
    let mut t = date_to_ydays(day, month, year) as i32;
    if tomorrow {
        t += 1;
    }

    // Advent starts:
    let advent1 = getadvent(year) as i32;
    // Christmas day-of-year
    let christmas = date_to_ydays(25, 12, year) as i32;
    let t_day = if tomorrow { day + 1 } else { day } as i32;

    // Once past the first Sunday of Advent, we're in Adv → Nat. The
    // sub-branches mirror Perl `Date.pm::getweek` lines 33-39 exactly:
    //
    //   Adv1..Adv4         from Advent-1 through Dec 24
    //   Nat25..Nat31       Dec 25-31, unpadded (file names Nat29.txt etc.)
    //
    // The Perl outer is `if ($t >= $advent1)`; the inner `< christmas`
    // gates *just* the Adv-week computation, not the whole branch.
    // An earlier port had the outer as `t >= advent1 && t < christmas`,
    // which silently fell through Dec 25-31 to the Pent24 branch.
    if t >= advent1 {
        if t < christmas {
            let n = 1 + (t - advent1) / 7;
            if month == 11 || day < 25 {
                return format!("Adv{}", n);
            }
        }
        return format!("Nat{}", t_day);
    }

    // Christmas Octave / pre-Epiphany days in January. Perl `sprintf("Nat%02i", $tDay)`
    // — zero-padded to 2 digits, matching the upstream file shape
    // (Nat02.txt, Nat05.txt, etc. — see vendor/divinum-officium/web/www/missa/Latin/Tempora/).
    let ordtime = 6 + 7 - day_of_week(6, 1, year) as i32;
    if month == 1 && (day as i32) < (ordtime - (tomorrow as i32)) {
        return format!("Nat{:02}", t_day);
    }

    // Easter
    let (e_day, e_month, _eyear) = geteaster(year);
    let easter_ydays = date_to_ydays(e_day, e_month, year) as i32;

    // Pre-Lent / Quadragesima
    if t < easter_ydays - 63 {
        let n = 1 + (t - ordtime) / 7;
        return format!("Epi{}", n);
    }
    if t < easter_ydays - 56 {
        return "Quadp1".to_string();
    }
    if t < easter_ydays - 49 {
        return "Quadp2".to_string();
    }
    if t < easter_ydays - 42 {
        return "Quadp3".to_string();
    }

    // Lent
    if t < easter_ydays {
        let n = 1 + (t - (easter_ydays - 42)) / 7;
        return format!("Quad{}", n);
    }

    // Eastertide
    if t < easter_ydays + 56 {
        let n = (t - easter_ydays) / 7;
        return format!("Pasc{}", n);
    }

    // Post-Pentecost
    let n = (t - (easter_ydays + 49)) / 7;
    if n < 23 {
        return format!("Pent{:02}", n);
    }
    // near end of year => pass to final logic
    let wd_dist = (advent1 - t + 6) / 7;
    if wd_dist < 2 {
        return "Pent24".to_string();
    }
    if n == 23 {
        return "Pent23".to_string();
    }
    // Possibly “EpiX” or “PentEpiX”
    if missa {
        // "PentEpiX"
        return format!("PentEpi{}", 8 - wd_dist);
    } else {
        // "EpiX"
        return format!("Epi{}", 8 - wd_dist);
    }
}

/// Handles the special “monthday” logic for older rubrics from August through December.
///
/// Returns a string like `"081-1"` meaning “August, first Sunday block / Monday”, etc.
/// Typically used to refine the Temporale in a bridging pattern. If out of range,
/// returns an empty string.
///
/// - `modernstyle`: toggles certain 1960 rubrical changes (shifting weeks).
/// - `tomorrow`: if true, we treat the date as the next day (used for Vesper logic).
///
/// The original code uses day-of-year calculations for months >= 7, then tries
/// to determine how many weeks have passed since the “first Sunday” in that month,
/// until a limit or until Advent begins. This is quite specific to certain rubrics.
///
/// Most uses of this logic in Divinum Officium are for partial expansions of August–December
/// ferias, bridging to an additional file like `Tempora/081-1.txt`.
///
/// ```
/// # use officium_rs::date::monthday;
/// let md = monthday(8, 9, 2023, false, false);
/// // Might return "081-1-2" or similar. The original code returns "081-1" with a suffix day-of-week.
/// if md.is_empty() {
///    // Not in that chunk
/// }
/// ```
pub fn monthday(
    day: u32,
    month: u32,
    year: i32,
    modernstyle: bool,
    tomorrow: bool
) -> String {
    // Only for months >= 7 in original code
    if month < 7 {
        return "".to_string();
    }
    let day_of_year = date_to_ydays(day, month, year);
    let mut base = day_of_year as i32;
    if tomorrow {
        base += 1;
    }

    // detect the first Sunday for each month from Aug=8..Dec=12
    // store those in an array for day-of-year, see how far we got.
    // If base < first_sunday, no result. If base >= that sunday => lit_month = that month
    let mut lit_month = 0;
    let mut first_sunday_day_of_year = Vec::new();
    for m in 8..=12 {
        // day-of-year for 1st of month
        let first_of_month = date_to_ydays(1, m, year);
        let dofweek = day_of_week(1, m, year);
        // This replicates: first_sunday_day_of_year = first_of_month - dofweek + 7 if dofweek >=4
        // in the original code, plus a condition if modernstyle => dofweek=0 => ...
        // The original code uses a repeated approach that tries to ensure the first Sunday is
        // actually the next Sunday if dofweek != 0. If dofweek=0 => Sunday => keep that day, else add (7 - dofweek).
        let mut sunday = first_of_month as i32 - dofweek as i32;
        if dofweek >= 4 || (dofweek != 0 && modernstyle) {
            sunday += 7;
        }
        first_sunday_day_of_year.push(sunday);
        if base >= sunday {
            lit_month = m as i32;
        } else {
            break;
        }
    }
    if lit_month == 0 {
        return "".to_string();
    }
    // If > 10 => might check Advent boundary
    if lit_month > 10 {
        let advent = getadvent(year) as i32;
        if base >= advent {
            return "".to_string();
        }
    }

    // figure out which index in `first_sunday_day_of_year` is for our lit_month
    let idx = (lit_month - 8) as usize;
    let day_of_week = day_of_week(day, month, year);
    let mut w = (base - first_sunday_day_of_year[idx]) / 7; // which week
    // special handling for October (10) + 1960 rubrics => skipping certain weeks
    if lit_month == 10 && modernstyle && w >= 2 {
        // The original logic: “the III. week vanishes in certain years”
        let offset = ydays_to_date(first_sunday_day_of_year[idx] as u32, year);
        let first_sunday_day = offset.0;
        // If that day is >=4 => skip the 3rd
        if first_sunday_day >= 4 {
            w += 1;
        }
    }
    // special handling for November
    if lit_month == 11 && (w > 0 || modernstyle) {
        let advent = getadvent(year) as i32;
        // The code uses 4 - floor((advent - base - 1)/7).
        // Then if modernstyle => skip the second week
        let alt_w = 4 - ((advent - base - 1) / 7);
        w = alt_w;
        if modernstyle && w == 1 {
            w = 0; // the II. week vanishes
        }
    }

    // Return format "MMW-WD" in the original code it's "081-1" but also appended day_of_week in advanced usage.
    // We'll produce the exact "MMW-W" format matching "082-2-3" or so. The original code ends up "081-1" for the week
    // but also references the day_of_week for special usage. Implementation details vary; we replicate the code's final string.
    // The original code ends with: "sprintf('%02i%01i-%01i', $lit_month, $week+1, $day_of_week)"
    format!(
        "{:02}{}-{}",
        lit_month,
        w + 1,
        day_of_week
    )
}

/// Returns the special Divinum Officium “Sancti” folder date string in `MM-DD` format.
/// This function adjusts for leap year in the historical sense: real Feb 24 → 02-29
/// (the bissextile day), and real Feb 25..29 → 02-24..28 (saints "deferred" so they
/// keep their original distance from March 1).
///
/// ```
/// # use officium_rs::date::get_sday;
/// assert_eq!(get_sday(2, 24, 2024), "02-29"); // real Feb 24 (leap) = bissextile
/// assert_eq!(get_sday(2, 25, 2024), "02-24"); // real Feb 25 (leap) = Matthias day
/// assert_eq!(get_sday(2, 24, 2025), "02-24"); // non-leap, unchanged
/// ```
pub fn get_sday(month: u32, day: u32, year: i32) -> String {
    let (m, d) = sday_pair(month, day, year);
    format!("{:02}-{:02}", m, d)
}

/// Same shift as `get_sday`, but returned as `(month, day)` so that
/// callers indexing `(u32, u32)` keys (`kalendarium_1570::lookup`,
/// `Corpus::sancti_entries`) don't need to re-parse a formatted
/// string.
///
/// Mirrors the upstream `DivinumOfficium::Date::get_sday` Perl: in a
/// leap year, real Feb 24 → kalendar 02-29 (the bissextile day), and
/// real Feb 25..29 → kalendar 02-24..28 (the saint-table days are
/// kept at their original distance from March 1, so they get
/// "deferred" by one day). Non-February dates pass through.
pub fn sday_pair(month: u32, day: u32, year: i32) -> (u32, u32) {
    if leap_year(year) && month == 2 {
        if day == 24 {
            return (2, 29);
        } else if day > 24 {
            return (2, day - 1);
        }
    }
    (month, day)
}

/// Sancti kalendar lookup key, or `None` when the date has no
/// kalendar entry under the current rubric's leap-year handling.
///
/// Mirrors `sday_pair`'s shift, with one extra rule for **leap
/// years only**: real Feb 23 returns `None`. The Vigil of
/// Matthias's "slot" at non-leap kalendar 02-23 is shadowed in
/// leap years: real Feb 23 falls through to ferial. The Vigil
/// itself is held at real Feb 24 via `sday_pair` → kalendar 02-29
/// (also the Vigil entry), where it can fire normally and be
/// outranked by higher-class temporal ferials (Quinquagesima
/// Tuesday rank 2.0 > Vigil 1.5).
///
/// Returns `Some(key)` for every other date.
pub fn sancti_kalendar_key(year: i32, month: u32, day: u32) -> Option<(u32, u32)> {
    if leap_year(year) && month == 2 && day == 23 {
        return None;
    }
    Some(sday_pair(month, day, year))
}

/// Returns the “Sancti/” date (MM-DD) for the *next* calendar day, used
/// especially for Vespers references. Internally increments the day-of-year.
/// For example, if “2023-02-28” -> next day is “2023-03-01” unless leap year logic etc.
///
/// This is simpler than a general date approach, because we only want the
/// “sancti day” string (which has special logic in leap years).
///
/// ```
/// # use officium_rs::date::nextday;
/// let next_s = nextday(2, 28, 2023); // => "03-01"
/// let next_sl = nextday(2, 28, 2024); // => "02-30" due to leap day logic in DO
/// ```
pub fn nextday(month: u32, day: u32, year: i32) -> String {
    let total = date_to_ydays(day, month, year) as i32 + 1;
    let max = if leap_year(year) { 366 } else { 365 };
    if total > max {
        // if we pass end of the year, jump to 1 Jan next year
        return get_sday(1, 1, year + 1);
    }
    let (d2, m2, y2) = ydays_to_date(total as u32, year);
    get_sday(m2, d2, y2)
}

/// Takes an original date string in `MM-DD-YYYY` format, shifts it by `inc` days
/// (which can be negative) and returns the new string, also in `MM-DD-YYYY`.
///
/// This function replicates the minimal arithmetic approach from the Perl code
/// in `prevnext`.
///
/// ```
/// # use officium_rs::date::prevnext;
/// let shifted = prevnext("02-26-2024", 2);
/// // => "02-28-2024", which is "02-30" in the Sancti sense, but here we keep the real calendar date.
/// assert_eq!(&shifted, "02-28-2024");
/// ```
pub fn prevnext(date_str: &str, inc: i32) -> String {
    // parse "MM-DD-YYYY"
    let parts = date_str.split('-').collect::<Vec<_>>();
    if parts.len() != 3 {
        // fallback
        return date_str.to_string();
    }
    let month = parts[0].parse::<i32>().unwrap_or(1);
    let day = parts[1].parse::<i32>().unwrap_or(1);
    let year = parts[2].parse::<i32>().unwrap_or(1970);

    let d = date_to_days(day as u32, month as u32, year) + inc;
    // If we go below day 0 => previous year, or above => next year, handled by `days_to_date`.
    if d < 0 {
        // all the way before 1970? The original code sets 31-12 (year-1).
        // But we keep consistent with days_to_date, which can handle earlier expansions.
        // Just rely on the logic that we won't go before the Gregorian threshold in normal usage.
    }

    let (_sec, _min, _hour, dday, dmonth, dyear, _wday, _yday, _dummy) = days_to_date(d);
    // Reconstruct "MM-DD-YYYY"
    format!("{:02}-{:02}-{:04}", dmonth + 1, dday, dyear + 1900)
}

/// Converts “days since 1970-01-01” (midnight-based) into a localtime-like tuple:
/// (sec, min, hour, day, month-1, year-1900, wday, yday, isdst=0).
///
/// This function is only needed for full backward compatibility with the
/// original approach used in `Date.pm`; in practice, a typical Rust program
/// would rely on standard library methods for date/time. However, if you are
/// replicating the old DO logic, it can be helpful.
///
/// *Warning:* The code tries to handle a wide range of years, but extreme
/// pre-1970 or post-2038 usage can lead to differences. The original code
/// also warns about the Gregorian calendar start.
///
/// ```
/// # use officium_rs::date::days_to_date;
/// let (sec, min, hour, dday, dmonth, dyear, wday, yday, isdst) = days_to_date(0);
/// // This is 1970-01-01 in the original logic
/// assert_eq!((dday, dmonth, dyear), (1, 0, 70));
/// ```
#[allow(clippy::too_many_arguments)]
pub fn days_to_date(days: i32) -> (i32, i32, i32, i32, i32, i32, i32, i32, i32) {
    // The original code reconstructs a date from the days offset, implementing
    // an algorithm that attempts to mimic localtime from a simple epoch. This
    // is valid for typical usage, though outside 1970–2038 range it’s strictly
    // an approximation. We replicate the logic exactly for consistency:
    //
    // Return: (sec, min, hour, day, month, year, wday, yday, isdst).
    // month in [0..11], year is offset from 1900.

    // (chrono fast path stripped; the manual fallback below covers the
    // full range we use.)

    // from the original code: if days < -141427 => error about pre-Gregorian
    if days < -141427 {
        // we won't panic by default, but we note it:
        // println!("Date before the Gregorian calendar is not well-defined in DO logic!");
    }

    // We'll initialize a local result akin to "6:00:00"
    let sec = 0;
    let min = 0;
    let hour = 6;
    let mut wday = (days + 4) % 7; // attempt to keep Sunday=0 offset
    if wday < 0 {
        wday += 7;
    }
    let isdst = 0;

    // Reconstruct year & day-of-year, then break down month, day.
    let (day, month, year) = days_to_date_fallback(days);

    // final
    let final_year = year - 1900;
    let final_mon = (month - 1) as i32;
    let final_day = day as i32;
    let final_wday = wday;
    let final_yday = date_to_ydays(day, month, year) as i32 - 1; // 0-based

    (
        sec,
        min,
        hour,
        final_day,
        final_mon,
        final_year,
        final_wday,
        final_yday,
        isdst,
    )
}

/// Fallback to replicate the manual logic for date-from-epoch in the original code,
/// parted out to keep `days_to_date` from being too large. Not intended for general use.
fn days_to_date_fallback(days: i32) -> (u32, u32, i32) {
    // This is a direct adaptation of the original big chunk of Perl code in `days_to_date`.
    // We'll keep the same structure for fidelity.

    // let d[6] = ...
    // We'll skip some steps about negative years. Just replicate carefully.

    // Start from year=2000 offset=10957 at c=20 in the original logic. We'll do the same.
    let mut count = 10957;
    let mut c = 20;
    let mut add: i32;

    if days < count {
        while days < count {
            c -= 1;
            add = if c % 4 == 0 { 36525 } else { 36524 };
            count -= add;
        }
    } else {
        while days >= count {
            add = if c % 4 == 0 { 36525 } else { 36524 };
            count += add;
            c += 1;
        }
        // revert one step:
        c -= 1;
        add = if c % 4 == 0 { 36525 } else { 36524 };
        count -= add;
    }

    let mut four_years = 4 * 365;
    if c % 4 == 0 {
        four_years += 1;
    }
    let mut yc = c * 100;
    while count <= days {
        let oldcount = count;
        let oldyc = yc;
        count += four_years;
        four_years = 4 * 365 + 1;
        yc += 4;
        if count > days {
            count = oldcount;
            yc = oldyc;
            break;
        }
    }
    let mut add2 = 366;
    if yc % 100 == 0 && yc % 400 != 0 {
        add2 = 365;
    }
    let mut year = yc;
    while count <= days {
        count += add2;
        add2 = 365;
        year += 1;
        if count > days {
            count -= add2;
            year -= 1;
            break;
        }
    }
    let is_leap = leap_year(year);
    let mut months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if is_leap {
        months[1] = 29;
    }
    let mut d = days - count + 1;
    let mut m = 0;
    while m < 12 && d > months[m] {
        d -= months[m];
        m += 1;
    }
    // final result
    (
        d as u32,
        (m + 1) as u32,
        year,
    )
}

/// Converts a date to “days since 1970-01-01 at 00:00:00” in the DO approach.
/// This is used mostly for “prevnext” or similar offset logic. As with
/// `days_to_date`, the code tries to handle older or future years in the same
/// style as the original script, which is not fully identical to standard Unix
/// timestamps for years < 1970 or > 2038, but suffices for the DO logic.
///
/// ```
/// # use officium_rs::date::date_to_days;
/// let days = date_to_days(1, 1, 1970);
/// assert_eq!(days, 0);
/// let days2 = date_to_days(2, 1, 1970);
/// assert_eq!(days2, 1);
/// ```
pub fn date_to_days(day: u32, month: u32, year: i32) -> i32 {
    // The original code uses a big chunk logic for wide year handling.
    // For year=1970 => "base point" => day=0.
    // We'll unify with the fallback approach used by `days_to_date`.
    // The difference is that for 1970..2038 it tries to do localtime quickly.
    // We'll just do it all with the same method for consistency.

    // We replicate:
    // - If within 1970..2038, we can do a simpler approach using chrono, returning the difference in days.
    // - Otherwise, we do manual year blocks.

    // (chrono fast path stripped; manual fallback handles all years.)
    date_to_days_fallback(day, month, year)
}

fn date_to_days_fallback(day: u32, month: u32, year: i32) -> i32 {
    // This reproduces the big chunk approach in Perl's `date_to_days`.
    // We define the “count” = 10957 at c=20 => year=2000 approach or so.
    // Then step up or down in centuries, then in 4-year blocks, then year by year,
    // then month by month, then day.

    // if we find year < ???, handle it carefully. The original code warns about
    // pre-Gregorian if we go below 1582, but does not forcibly stop.

    let mut c = 20;
    let mut ret = 10957; // day-of-year for 2000-01-01 relative to 1970? The original code picks such a pivot.
    let mut add: i32;

    // Move c to the century for `year`.
    let yc = year / 100;

    if yc < c {
        while c > yc {
            c -= 1;
            add = if c % 4 == 0 { 36525 } else { 36524 };
            ret -= add;
        }
    } else {
        while c < yc {
            add = if c % 4 == 0 { 36525 } else { 36524 };
            ret += add;
            c += 1;
        }
    }

    // Now handle the partial century inside that c * 100..(c+1)*100 range
    let mut four_years = 4 * 365;
    let mut leftover_years = (yc * 100) as i32;
    if leftover_years % 4 == 0 {
        four_years += 1;
    }
    while leftover_years < year - (year % 4) {
        ret += four_years;
        four_years = 4 * 365 + 1;
        leftover_years += 4;
    }

    // Now handle from leftover_years up to the actual year
    let mut add2 = 366;
    if leftover_years % 100 == 0 && leftover_years % 400 != 0 {
        add2 = 365;
    }
    let mut y2 = leftover_years;
    while y2 < year {
        ret += add2;
        add2 = 365;
        y2 += 1;
    }

    // We now add months
    let is_leap = leap_year(year);
    let mut months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if is_leap {
        months[1] = 29;
    }
    for m in 0..(month - 1) {
        ret += months[m as usize];
    }
    // Finally, add (day - 1)
    ret += (day - 1) as i32;

    // done
    ret
}

#[cfg(test)]
mod phase2_tests {
    //! Phase 2 calibration tests pinning the Rust `getweek` /
    //! `getadvent` / `geteaster` / `day_of_week` outputs against
    //! the upstream Perl `Date.pm`. Each assertion was emitted by
    //! `scripts/perl_getweek_year.pl` and cross-checked with the
    //! `getweek-check` binary across years 1900-2100 × 4 flag
    //! combinations (no divergences).
    //!
    //! Categories covered (per DIVINUM_OFFICIUM_PORT_PLAN.md Phase 2):
    //!   * Easter and Easter Octave
    //!   * Pentecost-Sunday transition (Pasc7 → Pent01)
    //!   * Pre-Lent (Quadp1..3) and Lent (Quad1..6)
    //!   * Eastertide (Pasc0..7)
    //!   * Post-Pentecost (Pent01..23) and the Pent24 cap
    //!   * PentEpi vs Epi switch on the `missa` flag
    //!   * Advent (Adv1..4)
    //!   * Christmas Octave Dec (Nat25..31, unpadded)
    //!   * Pre-Epiphany Jan (Nat01..09, zero-padded)
    //!   * Christmas-on-Sunday Advent shift (regression for 2022)
    use super::*;

    fn wk(d: u32, m: u32, y: i32) -> String {
        getweek(d, m, y, false, true)
    }

    fn wk_office(d: u32, m: u32, y: i32) -> String {
        getweek(d, m, y, false, false)
    }

    #[test]
    fn easter_dates_across_decade() {
        // Source: any Computus table; cross-checked with Perl harness.
        assert_eq!(geteaster(2024), (31, 3, 2024));
        assert_eq!(geteaster(2025), (20, 4, 2025));
        assert_eq!(geteaster(2026), (5,  4, 2026));
        assert_eq!(geteaster(2027), (28, 3, 2027));
        assert_eq!(geteaster(2028), (16, 4, 2028));
    }

    #[test]
    fn easter_sunday_emits_pasc0() {
        for (d, m, y) in [(31, 3, 2024), (20, 4, 2025), (5, 4, 2026),
                          (28, 3, 2027), (16, 4, 2028)] {
            assert_eq!(wk(d, m, y), "Pasc0", "Easter {y}-{m:02}-{d:02}");
        }
    }

    #[test]
    fn eastertide_progression_2026() {
        // Easter 2026 = April 5. Successive Sundays:
        assert_eq!(wk( 5, 4, 2026), "Pasc0"); // Easter Sunday
        assert_eq!(wk(12, 4, 2026), "Pasc1");
        assert_eq!(wk(19, 4, 2026), "Pasc2");
        assert_eq!(wk(26, 4, 2026), "Pasc3");
        assert_eq!(wk( 3, 5, 2026), "Pasc4");
        assert_eq!(wk(10, 5, 2026), "Pasc5");
        assert_eq!(wk(17, 5, 2026), "Pasc6");
        assert_eq!(wk(24, 5, 2026), "Pasc7"); // Pentecost Sunday
        assert_eq!(wk(31, 5, 2026), "Pent01"); // Trinity Sunday
    }

    #[test]
    fn pre_lent_and_lent_2026() {
        // 2026: Septuagesima Sunday = Feb 1 (9 weeks before Easter).
        assert_eq!(wk(25, 1, 2026), "Epi3");
        assert_eq!(wk( 1, 2, 2026), "Quadp1"); // Septuagesima
        assert_eq!(wk( 8, 2, 2026), "Quadp2"); // Sexagesima
        assert_eq!(wk(15, 2, 2026), "Quadp3"); // Quinquagesima
        assert_eq!(wk(22, 2, 2026), "Quad1"); // 1st Sun of Lent
        assert_eq!(wk( 1, 3, 2026), "Quad2");
        assert_eq!(wk(29, 3, 2026), "Quad6"); // Palm Sunday
    }

    #[test]
    fn advent_progression_2026() {
        assert_eq!(wk(29, 11, 2026), "Adv1");
        assert_eq!(wk( 6, 12, 2026), "Adv2");
        assert_eq!(wk(13, 12, 2026), "Adv3");
        assert_eq!(wk(20, 12, 2026), "Adv4");
        assert_eq!(wk(24, 12, 2026), "Adv4"); // last day before Christmas
    }

    #[test]
    fn christmas_octave_unpadded() {
        // Dec 25-31 → Nat25..Nat31 with no leading zero (file names
        // are Nat29.txt etc.).
        assert_eq!(wk(25, 12, 2026), "Nat25");
        assert_eq!(wk(26, 12, 2026), "Nat26");
        assert_eq!(wk(27, 12, 2026), "Nat27");
        assert_eq!(wk(28, 12, 2026), "Nat28");
        assert_eq!(wk(29, 12, 2026), "Nat29");
        assert_eq!(wk(30, 12, 2026), "Nat30");
        assert_eq!(wk(31, 12, 2026), "Nat31");
    }

    #[test]
    fn pre_epiphany_jan_zero_padded() {
        // Jan 1-? → Nat01..Nat0X zero-padded (file names Nat02.txt
        // through Nat05.txt confirm the convention).
        assert_eq!(wk(1, 1, 2026), "Nat01");
        assert_eq!(wk(2, 1, 2026), "Nat02");
        assert_eq!(wk(3, 1, 2026), "Nat03");
        assert_eq!(wk(4, 1, 2026), "Nat04");
        assert_eq!(wk(5, 1, 2026), "Nat05");
        assert_eq!(wk(6, 1, 2026), "Nat06");
    }

    #[test]
    fn christmas_on_sunday_advent_shift_2022() {
        // 2022: Christmas falls on Sunday → 1st Sunday of Advent
        // is Nov 27 (4 Sundays back, *not* Dec 4). Pre-Phase-2 port
        // dropped the `dow || 7` Perl trick and shifted Advent by 7
        // days for years like 2017/2022/2033/2039.
        assert_eq!(wk(27, 11, 2022), "Adv1");
        assert_eq!(wk( 4, 12, 2022), "Adv2");
        assert_eq!(wk(11, 12, 2022), "Adv3");
        assert_eq!(wk(18, 12, 2022), "Adv4");
        assert_eq!(wk(25, 12, 2022), "Nat25");
    }

    #[test]
    fn pent_epi_vs_epi_split_on_missa_flag() {
        // 2027: Easter very early (Mar 28), so post-Pentecost weeks
        // overflow Pent24 by November. The `missa` flag selects:
        //   true  → "PentEpi{n}"
        //   false → "Epi{n}"
        // (file `web/www/missa/Latin/Tempora/PentEpi6.txt` exists.)
        assert_eq!(wk       ( 1, 11, 2027), "PentEpi4");
        assert_eq!(wk_office( 1, 11, 2027), "Epi4");
        assert_eq!(wk       ( 7, 11, 2027), "PentEpi5");
        assert_eq!(wk_office( 7, 11, 2027), "Epi5");
        assert_eq!(wk       (14, 11, 2027), "PentEpi6");
        assert_eq!(wk_office(14, 11, 2027), "Epi6");
        // Pent24 cap kicks in on the last week before Advent.
        assert_eq!(wk       (21, 11, 2027), "Pent24");
        assert_eq!(wk_office(21, 11, 2027), "Pent24");
    }

    #[test]
    fn day_of_week_pinned_anchors() {
        // 0 = Sun, 6 = Sat
        assert_eq!(day_of_week( 5, 4, 2026), 0); // Easter Sunday 2026
        assert_eq!(day_of_week(25, 12, 2022), 0); // Christmas Sunday
        assert_eq!(day_of_week(25, 12, 2026), 5); // Christmas Friday
        assert_eq!(day_of_week(29, 4, 2026), 3); // Wed (Pasc3)
        assert_eq!(day_of_week( 1, 1, 2000), 6); // Sat
    }

    #[test]
    fn getadvent_year_shape() {
        // 1st Sunday of Advent as a day-of-year. Hand-checked
        // against a calendar for the Christmas-on-Sunday case (2022)
        // and the Christmas-on-Friday case (2026).
        // 2022: Advent-1 = Nov 27 = day-of-year 331.
        assert_eq!(getadvent(2022), 331);
        // 2026: Advent-1 = Nov 29 = day-of-year 333 (non-leap).
        assert_eq!(getadvent(2026), 333);
        // 2024: Christmas Wed → Advent-1 = Dec 1 = day-of-year 336 (leap).
        assert_eq!(getadvent(2024), 336);
    }
}

#[cfg(test)]
mod leap_shift_tests {
    //! Pin the Tridentine leap-year shift in `sday_pair` against
    //! upstream Perl `Date.pm::get_sday`. The leap day is "kept" on
    //! Feb 24 (kalendar key 02-29 — bissextile day, encoded as the
    //! Vigil of Matthias); subsequent days shift down by one so saint
    //! tables stay at their original distance from March 1.
    use super::*;

    #[test]
    fn non_leap_passes_through() {
        // Non-leap: no shift, ever.
        for d in 1..=31 {
            assert_eq!(sday_pair(2, d, 2025), (2, d));
        }
        for d in 1..=31 {
            assert_eq!(sday_pair(3, d, 2025), (3, d));
        }
    }

    #[test]
    fn leap_february_shift() {
        // Real Feb 1..23 in a leap year keep their key (no shift).
        for d in 1..=23 {
            assert_eq!(sday_pair(2, d, 2024), (2, d));
        }
        // Real Feb 24 (leap) → kalendar key 02-29 (the inserted
        // bissextile day, encoded with the Vigil of Matthias).
        assert_eq!(sday_pair(2, 24, 2024), (2, 29));
        // Real Feb 25..29 (leap) shift DOWN by one: real Feb 25 ⇒
        // kalendar 02-24 (Matthias's day), real Feb 26 ⇒ 02-25, etc.
        assert_eq!(sday_pair(2, 25, 2024), (2, 24));
        assert_eq!(sday_pair(2, 26, 2024), (2, 25));
        assert_eq!(sday_pair(2, 27, 2024), (2, 26));
        assert_eq!(sday_pair(2, 28, 2024), (2, 27));
        assert_eq!(sday_pair(2, 29, 2024), (2, 28));
    }

    #[test]
    fn non_february_unaffected_in_leap_year() {
        assert_eq!(sday_pair(1, 24, 2024), (1, 24));
        assert_eq!(sday_pair(3, 1, 2024), (3, 1));
        assert_eq!(sday_pair(3, 25, 2024), (3, 25));
    }

    #[test]
    fn century_leap_year_handling() {
        // 2000: leap (divisible by 400).
        assert_eq!(sday_pair(2, 24, 2000), (2, 29));
        // 1900: NOT leap (divisible by 100, not 400).
        assert_eq!(sday_pair(2, 24, 1900), (2, 24));
        // 2100: NOT leap.
        assert_eq!(sday_pair(2, 24, 2100), (2, 24));
    }

    #[test]
    fn get_sday_string_round_trip() {
        // The string-returning sibling stays in sync with sday_pair.
        assert_eq!(get_sday(2, 24, 2024), "02-29");
        assert_eq!(get_sday(2, 25, 2024), "02-24");
        assert_eq!(get_sday(2, 24, 2025), "02-24");
    }

    #[test]
    fn sancti_kalendar_key_suppresses_leap_feb_23() {
        // C5 fix: leap-year Feb 23 has no kalendar entry — the Vigil
        // of Matthias is held on real Feb 24 (kalendar 02-29) under
        // the leap-year intercalation. Without this suppression the
        // Vigil fires twice (Feb 23 AND Feb 24) and 02-23 wrongly
        // wins on Feb 23.
        assert_eq!(sancti_kalendar_key(2024, 2, 23), None);
        assert_eq!(sancti_kalendar_key(2000, 2, 23), None);
        assert_eq!(sancti_kalendar_key(2060, 2, 23), None);
    }

    #[test]
    fn sancti_kalendar_key_passes_through_non_leap() {
        // Non-leap years: Feb 23 is the Vigil of Matthias as usual.
        assert_eq!(sancti_kalendar_key(2025, 2, 23), Some((2, 23)));
        assert_eq!(sancti_kalendar_key(1900, 2, 23), Some((2, 23)));
        assert_eq!(sancti_kalendar_key(2100, 2, 23), Some((2, 23)));
    }

    #[test]
    fn sancti_kalendar_key_leap_feb_24_resolves_to_bissextile() {
        // Real Feb 24 in leap years routes to kalendar 02-29 (the
        // bissextile day encoded as the Vigil). The suppression rule
        // does NOT cover Feb 24 — under 1570 the Vigil correctly
        // fires here when not preempted by a higher-rank Tempora
        // ferial.
        assert_eq!(sancti_kalendar_key(2024, 2, 24), Some((2, 29)));
        assert_eq!(sancti_kalendar_key(2000, 2, 24), Some((2, 29)));
    }

    #[test]
    fn sancti_kalendar_key_leap_post_feb_24_shifts() {
        // Feb 25..29 in leap years shift down by one (saint table
        // stays at original distance from March 1).
        assert_eq!(sancti_kalendar_key(2024, 2, 25), Some((2, 24)));
        assert_eq!(sancti_kalendar_key(2024, 2, 28), Some((2, 27)));
        assert_eq!(sancti_kalendar_key(2024, 2, 29), Some((2, 28)));
    }

    #[test]
    fn sancti_kalendar_key_non_february_unchanged() {
        for (m, d, y) in [(1, 23, 2024), (3, 23, 2024), (12, 24, 2024)] {
            assert_eq!(
                sancti_kalendar_key(y, m, d),
                Some((m, d)),
                "non-Feb date should pass through: {y}-{m:02}-{d:02}"
            );
        }
    }
}
