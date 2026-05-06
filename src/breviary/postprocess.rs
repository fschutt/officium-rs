//! Postprocess — text-level scrubs and wrappers.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/horas.pl`:
//!
//! - `resolve_refs($t, $lang)` (lines 89-212) — outer text walker.
//!   Splits the body into lines, expands `$<name>` and `&<name>` refs,
//!   applies the per-line "red prefix" / "large chapter" / "first
//!   letter initial" scrubs.
//! - `adjust_refs($name, $lang)` (lines 294-325) — rewrite a macro
//!   reference based on `$rule` (Requiem-gloria swap, Triduum
//!   gloria-omission, priest-vs-non-priest Dominus_vobiscum branch).
//! - `setlink($name, $ind, $lang)` (lines 329-440) — embed a link to
//!   a popup / expand-this-section action. UI-side; the Rust port
//!   emits structured `RenderedLine::Link` instead.
//! - `get_link_name($name)` (line 441) — translate a macro name to
//!   its display label, with rubric-conditional substitutions
//!   (`&Gloria1` → `&gloria` etc.).
//! - `setasterisk($line)` (lines 606-650) — psalm-verse asterisk
//!   placement (the breath-pause `*`).
//! - `getantcross($psalmline, $antline)` (lines 240-278) — Tridentine
//!   `‡` dagger marker on psalm verses that begin a new psalm
//!   subdivision.
//! - `depunct($item)` (line 280) — strip punctuation + de-accent
//!   for antiphon-vs-verse comparison.
//! - `columnsel($lang)` (line 652) — second-column language
//!   selection. Single-column always in the Rust port → identity
//!   helper.
//! - `postprocess_ant($ant, $lang)` (line 660) — antiphon-end period
//!   + Paschal Alleluia injection.
//! - `postprocess_vr($vr, $lang)` (line 680) — versicle/response
//!   Paschal Alleluia injection.
//! - `postprocess_short_resp($capit, $lang)` (line 697) — short
//!   responsory body Alleluia injection.
//! - `alleluia_required($dayname, $votive)` (line 729) — predicate.

use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Top-level body-of-section walker. Splits a body into lines,
/// expands `$<name>` / `&<name>` refs, applies per-line scrubs.
///
/// Mirror of `resolve_refs($t, $lang)` lines 89-212.
pub fn resolve_refs(_office: &OfficeOutput, _body: &str) -> Vec<RenderedLine> {
    // TODO(B20): port horas.pl:89-212.
    unimplemented!("phase B20: resolve_refs")
}

/// Rewrite a macro reference based on the active rule body.
///
/// Mirror of `adjust_refs($name, $lang)` lines 294-325. Specific
/// rewrites:
///   - `&Gloria` + `Requiem gloria` rule → `$Requiem`
///   - `&Gloria` + Triduum → `Gloria omittitur` rubric line
///   - `&Dominus_vobiscum1` + non-priest + Preces Dominicales →
///     `prayer('Dominus')` line 4 ("Domine, exaudi orationem meam")
///   - `&Dominus_vobiscum2` + non-priest → same as above
pub fn adjust_refs(_office: &OfficeOutput, _name: &str) -> String {
    // TODO(B20): port horas.pl:294-325.
    unimplemented!("phase B20: adjust_refs")
}

/// Insert the breath-pause asterisk into a psalm verse.
///
/// Mirror of `setasterisk($line)` lines 606-650. The decision is
/// length-based with two passes:
///
/// 1. **Punctuation pass:** if the line contains `.`/`:`/`;`/`?`/`!`
///    AND the tail after the FIRST such punctuation is longer than
///    `lp2`, walk back from the right to find the LAST such punctuation
///    where the tail length crosses the `lp2` threshold; insert the
///    asterisk there.
///
/// 2. **Comma / space fallback:** if no punctuation split fired, try
///    again splitting on commas (when the comma-tail is long enough)
///    or on whitespace (otherwise). Includes a "long-word coalescing"
///    inner loop that pulls short trailing words into the tail to
///    avoid awkward `* word` splits.
///
/// The threshold `lp2` is:
///   * 24 chars when the line is > 64 chars long (long-verse mode)
///   * 6 chars when the line is < 24 chars long (short-verse mode)
///   * 12 chars otherwise (default)
///
/// Already-asterisked lines (where `*` is followed by 10+ chars of
/// tail) pass through unchanged — useful for caller pre-formatting.
pub fn set_asterisk(line: &str) -> String {
    // Strip trailing whitespace.
    let line = line.trim_end();

    // Already has an asterisk with a meaningful tail (>9 chars after
    // the first `*`)? Pass through.
    if let Some(idx) = line.find('*') {
        let tail = &line[idx + 1..];
        if tail.chars().count() > 9 {
            return line.to_string();
        }
    }

    let line_len = line.chars().count();
    let lp2: usize = if line_len > 64 {
        24
    } else if line_len < 24 {
        6
    } else {
        12
    };

    // Pass 1: punctuation-based split.
    if let Some(out) = punctuation_split(line, lp2) {
        return out;
    }

    // Pass 2: comma or space split.
    comma_or_space_split(line, lp2)
}

/// Try splitting on `.`/`:`/`;`/`?`/`!` boundaries. Returns the
/// formatted line on success, `None` to fall back to comma/space.
fn punctuation_split(line: &str, lp2: usize) -> Option<String> {
    // Find the first punctuation; if the tail after it is short
    // enough, we don't need this pass at all.
    let punct_chars: &[char] = &['.', ':', ';', '?', '!'];
    let first = line.find(punct_chars)?;
    let after_first = &line[first + 1..];
    if char_count(after_first) <= lp2 {
        return None;
    }

    // Walk back from the end, accumulating `t` (the tail) until
    // `(after + t).len() > lp2`.
    let mut l = line.to_string();
    let mut t = String::new();
    loop {
        let (head_owned, breaker, after_owned) = match split_last_punct(&l, punct_chars) {
            Some((h, b, a)) => (h.to_string(), b, a.to_string()),
            None => break,
        };
        let new_l = head_owned;
        let combined_tail = format!("{after_owned}{t}");
        if char_count(&combined_tail) > lp2 {
            if char_count(&new_l) > lp2 {
                return Some(format!("{new_l}{breaker} *{combined_tail}"));
            }
            break;
        }
        t = format!("{breaker}{after_owned}{t}");
        l = new_l;
    }
    None
}

/// Split off the last occurrence of any of `punct_chars` in `line`,
/// returning `(head_before, breaker_char, tail_after)`. Returns
/// `None` when no match.
fn split_last_punct<'a>(line: &'a str, punct_chars: &[char]) -> Option<(&'a str, char, &'a str)> {
    let mut last: Option<(usize, char)> = None;
    for (i, c) in line.char_indices() {
        if punct_chars.contains(&c) {
            last = Some((i, c));
        }
    }
    let (i, c) = last?;
    let head = &line[..i];
    let after_byte = i + c.len_utf8();
    let after = &line[after_byte..];
    Some((head, c, after))
}

fn comma_or_space_split(line: &str, lp2: usize) -> String {
    // Pick comma if tail-after-first-comma is long enough; else space.
    let mut breaker = if let Some(idx) = line.find(',') {
        if char_count(&line[idx + 1..]) > lp2 {
            ','
        } else {
            ' '
        }
    } else {
        ' '
    };

    let mut l = line.to_string();
    let mut t = String::new();
    loop {
        // Find the LAST occurrence of `breaker` in `l`.
        let split_idx = match l.rfind(breaker) {
            Some(i) => i,
            None => break,
        };
        let head_owned = l[..split_idx].to_string();
        let after_byte = split_idx + breaker.len_utf8();
        let mut after = l[after_byte..].to_string();

        // Comma fallback: if head got too short under comma-mode,
        // restart in space-mode.
        if char_count(&head_owned) < lp2 && breaker == ',' {
            breaker = ' ';
            l = line.to_string();
            t = String::new();
            continue;
        }

        // Inner long-word-coalesce loop: while breaker is space AND
        // head is too long AND last word of head is short (<4 chars),
        // pull the last word into `after`.
        let mut coalesce_l = head_owned;
        if breaker == ' ' {
            loop {
                if char_count(&coalesce_l) <= lp2 + 3 {
                    break;
                }
                let last_space = match coalesce_l.rfind(' ') {
                    Some(i) => i,
                    None => break,
                };
                let last_word = coalesce_l[last_space + 1..].to_string();
                if char_count(&last_word) >= 4 {
                    break;
                }
                let new_after = format!("{last_word} {after}");
                coalesce_l = coalesce_l[..last_space].to_string();
                after = new_after;
            }
        }

        let combined_tail = format!("{after}{t}");
        if char_count(&combined_tail) > lp2 {
            // Ensure leading space on `after` if missing.
            let after_for_out = if combined_tail.starts_with(' ') {
                combined_tail
            } else {
                format!(" {combined_tail}")
            };
            return format!("{coalesce_l}{breaker}*{after_for_out}");
        }
        t = format!("{breaker}{after}{t}");
        l = coalesce_l;
    }
    let leading = if t.starts_with(' ') {
        t
    } else {
        format!(" {t}")
    };
    format!("{l} *{leading}")
}

fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Insert the Tridentine `‡` dagger marker on antiphon-matching
/// verses. Returns the modified psalm line. Mirror of `getantcross`
/// lines 240-278.
pub fn get_ant_cross(_psalm_line: &str, _ant_line: &str) -> String {
    // TODO(B20): port horas.pl:240-278.
    unimplemented!("phase B20: get_ant_cross")
}

/// Strip punctuation + diacritics for antiphon/verse comparison.
/// Mirror of `depunct($item)` line 280-292.
///
/// Used by [`get_ant_cross`] to compare antiphon words against psalm-
/// verse words case-insensitively across diacritics.
///
/// Transformations (in order):
///   1. Remove punctuation: `. , : ? ! " ' ; * ( )`
///   2. Fold accented Latin vowels to their base forms:
///      á/Á → a, é/É → e, í/Í → i, ó/ö/õ/Ó/Ö/Ô → o, ú/ü/û/Ú/Ü/Û → u
///   3. Substitute J/j → I/i (Latin convention)
///   4. Expand ligatures: æ → ae, œ → oe
pub fn depunct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '.' | ',' | ':' | '?' | '!' | '"' | '\'' | ';' | '*' | '(' | ')' => {}
            'á' | 'Á' => out.push('a'),
            'é' | 'É' => out.push('e'),
            'í' | 'Í' => out.push('i'),
            'ó' | 'ö' | 'õ' | 'Ó' | 'Ö' | 'Ô' => out.push('o'),
            'ú' | 'ü' | 'û' | 'Ú' | 'Ü' | 'Û' => out.push('u'),
            'J' => out.push('I'),
            'j' => out.push('i'),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            other => out.push(other),
        }
    }
    out
}

/// Postprocess one antiphon body. Mirror of `postprocess_ant`
/// line 660.
pub fn postprocess_ant(_office: &OfficeOutput, _ant: &mut String) {
    // TODO(B20): port horas.pl:660-676.
    // Two scrubs:
    //   1. Append a period if the antiphon doesn't end in one.
    //   2. Inject a single Paschal Alleluia under
    //      `alleluia_required && lang != gabc`.
    unimplemented!("phase B20: postprocess_ant")
}

/// Postprocess a versicle/response pair. Mirror of `postprocess_vr`
/// line 680.
pub fn postprocess_vr(_office: &OfficeOutput, _vr: &mut String) {
    // TODO(B20): port horas.pl:680-694.
    unimplemented!("phase B20: postprocess_vr")
}

/// Postprocess a short responsory body. Mirror of
/// `postprocess_short_resp` line 697.
pub fn postprocess_short_resp(_office: &OfficeOutput, _capit: &mut Vec<RenderedLine>) {
    // TODO(B20): port horas.pl:697-728.
    unimplemented!("phase B20: postprocess_short_resp")
}

/// Predicate: should the active office add a Paschal Alleluia to
/// antiphons / versicles / short responsories?
///
/// Mirror of `alleluia_required($dayname, $votive)` line 729-734:
///
/// ```perl
/// $dayname =~ /Pasc/i && $votive !~ /C(?:9|12)/;
/// ```
///
/// Returns true when:
///   - dayname[0] starts with `Pasc` (Paschaltide), AND
///   - votive is NOT C9 (Office of the Dead) and NOT C12 (BMV Parva).
///
/// Until B10a wires `OfficeInput::votive` through to `OfficeOutput`,
/// this function takes the votive code as an explicit parameter.
/// Pass `""` (or any other non-C9/C12 string) when no votive is in
/// effect.
pub fn alleluia_required(office: &OfficeOutput, votive: &str) -> bool {
    use crate::core::Season;
    matches!(office.season, Season::Easter)
        && !votive.contains("C9")
        && !votive.contains("C12")
}

/// Inject a single "alleluia" at the end of a body if it isn't
/// already present. Mirror of upstream `LanguageTextTools::ensure_single_alleluia`.
pub fn ensure_single_alleluia(_body: &mut String) {
    // TODO(B20): port LanguageTextTools::ensure_single_alleluia.
    // Mass-side may already have an equivalent; reuse if so.
    unimplemented!("phase B20: ensure_single_alleluia")
}

/// Inject a double "alleluia, alleluia" at the end of a body if it
/// isn't already present. Mirror of upstream
/// `LanguageTextTools::ensure_double_alleluia`.
pub fn ensure_double_alleluia(_body: &mut String) {
    // TODO(B20): port LanguageTextTools::ensure_double_alleluia.
    unimplemented!("phase B20: ensure_double_alleluia")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── depunct ────────────────────────────────────────────────

    #[test]
    fn depunct_strips_punctuation() {
        assert_eq!(depunct("Beatus vir, qui non abiit."), "Beatus vir qui non abiit");
        assert_eq!(
            depunct("(quis dabit?) — quia non!"),
            "quis dabit — quia non"
        );
        assert_eq!(depunct("'singles' \"doubles\""), "singles doubles");
    }

    #[test]
    fn depunct_folds_accents() {
        assert_eq!(depunct("Beátus vir"), "Beatus vir");
        assert_eq!(depunct("órat"), "orat");
        assert_eq!(depunct("túus"), "tuus");
        // Accent-folds always produce LOWERCASE base — matches Perl
        // `s/[óöõÓÖÔ]/o/g` which always substitutes with literal
        // lowercase `o` regardless of input case. Bug-for-bug parity.
        // Non-accented characters (D/l/etc) preserve their case.
        assert_eq!(depunct("DÓminus"), "Dominus");
        assert_eq!(depunct("Élatus"), "elatus");
        assert_eq!(depunct("Á"), "a");
        assert_eq!(depunct("Cantátes"), "Cantates");
    }

    #[test]
    fn depunct_substitutes_j_to_i() {
        assert_eq!(depunct("Jesus Justus"), "Iesus Iustus");
        assert_eq!(depunct("majestas"), "maiestas");
    }

    #[test]
    fn depunct_expands_ligatures() {
        assert_eq!(depunct("cæli"), "caeli");
        assert_eq!(depunct("œcumenicus"), "oecumenicus");
        assert_eq!(depunct("præfatio cælestis"), "praefatio caelestis");
    }

    #[test]
    fn depunct_preserves_other_characters() {
        // Characters not in the substitution table pass through.
        assert_eq!(depunct("ABCdef 123"), "ABCdef 123");
        assert_eq!(depunct(""), "");
    }

    // ─── alleluia_required ──────────────────────────────────────

    fn office_with_season(season: crate::core::Season) -> OfficeOutput {
        use crate::core::*;
        OfficeOutput {
            date: Date::new(2026, 4, 5),
            rubric: Rubric::Tridentine1570,
            winner: FileKey::parse("Tempora/Pasc1-0"),
            commemoratio: None,
            scriptura: None,
            commune: None,
            commune_type: CommuneType::None,
            rank: Rank {
                class: RankClass::First,
                kind: RankKind::DuplexIClassis,
                raw_label: String::new(),
                rank_num: 7.0,
            },
            rule: vec![],
            day_kind: DayKind::Sunday,
            season,
            color: Color::White,
            vespers_split: None,
            reform_trace: vec![],
        }
    }

    #[test]
    fn alleluia_required_only_in_easter() {
        use crate::core::Season;
        let easter = office_with_season(Season::Easter);
        assert!(alleluia_required(&easter, ""));
        let advent = office_with_season(Season::Advent);
        assert!(!alleluia_required(&advent, ""));
        let lent = office_with_season(Season::Lent);
        assert!(!alleluia_required(&lent, ""));
    }

    #[test]
    fn alleluia_required_suppressed_under_c9_or_c12_votive() {
        use crate::core::Season;
        let easter = office_with_season(Season::Easter);
        // Office of the Dead — no Alleluia even in Easter.
        assert!(!alleluia_required(&easter, "C9"));
        // BMV Parva — same.
        assert!(!alleluia_required(&easter, "C12"));
        // Other votive — Alleluia OK.
        assert!(alleluia_required(&easter, "C7"));
        assert!(alleluia_required(&easter, ""));
    }

    // ─── set_asterisk ───────────────────────────────────────────

    #[test]
    fn set_asterisk_passes_through_already_asterisked() {
        // Already has `*` followed by 10+ chars of tail → unchanged.
        let line = "Beatus vir qui * non abiit in consilio impiorum";
        assert_eq!(set_asterisk(line), line);
    }

    #[test]
    fn set_asterisk_short_line_uses_space_split() {
        // Short line (<24 chars): lp2 = 6. Splits at space such that
        // the tail is > 6 chars.
        let result = set_asterisk("Beatus vir qui non abiit");
        // Should contain `*` somewhere, with both halves non-empty.
        assert!(result.contains('*'), "no asterisk in: {result:?}");
        let parts: Vec<&str> = result.split('*').collect();
        assert_eq!(parts.len(), 2, "expected exactly one asterisk: {result:?}");
        assert!(!parts[0].trim().is_empty());
        assert!(!parts[1].trim().is_empty());
    }

    #[test]
    fn set_asterisk_uses_punctuation_when_available() {
        // Line with a colon and a long tail after it → split at the colon.
        let line = "Cantate Domino canticum novum: cantate Domino omnis terra";
        let result = set_asterisk(line);
        assert!(result.contains(": *") || result.contains(":*"));
    }

    #[test]
    fn set_asterisk_strips_trailing_whitespace() {
        let result = set_asterisk("Beatus vir qui   ");
        // No trailing whitespace in the middle of the result.
        assert!(!result.ends_with("   "));
    }

    #[test]
    fn set_asterisk_produces_balanced_split_for_average_line() {
        // Average length, no internal punctuation: expects asterisk
        // somewhere between the middle and end.
        let line = "in domo Domini ambulabamus cum consensu";
        let result = set_asterisk(line);
        assert!(result.contains('*'));
        // Verify the verb head and tail portions don't go missing.
        assert!(result.contains("Domini") || result.contains("ambulabamus"));
    }

    #[test]
    fn set_asterisk_preserves_unicode() {
        // Latin diacritics are multi-byte UTF-8; the function must
        // operate on chars, not bytes.
        let line = "Beátus vir qui non ábiit in consílio impiórum";
        let result = set_asterisk(line);
        assert!(result.contains('*'));
        assert!(result.contains("Beátus"));
        assert!(result.contains("impiórum"));
    }

    #[test]
    fn set_asterisk_long_line_uses_lp2_24() {
        // Long line (>64 chars): lp2 = 24. Split should land later
        // in the line.
        let line = "Et facta est super eum manus Domini, et locutus est ad Ezechielem prophetam Ego Dominus Deus tuus";
        let result = set_asterisk(line);
        assert!(result.contains('*'));
    }
}
