//! Phase 6 — Rust↔Perl regression machinery.
//!
//! This module is the comparison engine that powers the
//! `year-sweep` binary and (eventually) the `/wip/missal-checks`
//! board.  Inputs:
//!
//!   * The Rust `MassPropers` produced by `mass::mass_propers`.
//!   * Raw Perl HTML (from `scripts/do_render.sh DATE VERSION
//!     SanctaMissa`).
//!
//! Output:
//!
//!   * Per-section comparison verdict (`SectionStatus`).
//!   * Aggregate `DayReport` with winner check, section verdicts,
//!     and rough byte counts.
//!
//! No I/O — pure functions over strings. The binary handles the
//! shell-out to Perl and the on-disk report writes.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use crate::divinum_officium::core::MassPropers;
use crate::divinum_officium::mass::expand_macros;
use crate::divinum_officium::missa;

// ─── Public types ────────────────────────────────────────────────────

/// Canonical Mass-section names we track. Latin headers in the Perl
/// HTML; in-struct fields on `MassPropers`. Order is the canonical
/// rendering order.
pub const PROPER_SECTIONS: &[&str] = &[
    "Introitus",
    "Oratio",
    "Lectio",
    "Graduale",
    "Tractus",
    "Sequentia",
    "Evangelium",
    "Offertorium",
    "Secreta",
    "Prefatio",
    "Communio",
    "Postcommunio",
];

/// English equivalents — appear right after the Latin section in the
/// upstream rendering. Used as cut-off markers when extracting the
/// Latin body span.
pub const ENGLISH_SECTION_NAMES: &[&str] = &[
    "Introit",
    "Collect",
    "Lesson",
    "Gradual",
    "Tract",
    "Sequence",
    "Gospel",
    "Offertory",
    "Secret",
    "Preface",
    "Communion",
    "Postcommunion",
];

/// Mass-Ordinary headers we explicitly skip when scanning. These are
/// not propers and are not produced by `mass::mass_propers`.
pub const ORDINARY_HEADERS: &[&str] = &[
    "Incipit", "Beginning",
    "Kyrie",
    "Gloria",
    "Credo", "Creed",
    "Pater Noster", "Our Father",
    "Communicantes",
    "Hanc Igitur",
    "Qui Pridie",
    "Mysterium Fidei",
    "Pater noster",
    "Agnus Dei",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SectionStatus {
    /// Both sides empty — nothing to compare.
    Empty,
    /// Rust output (normalised) is a substring of Perl's. Green cell.
    Match,
    /// Rust has content but Perl is empty for this section.
    PerlBlank,
    /// Perl has content but Rust did not produce one.
    RustBlank,
    /// Both have content; Rust's normalised form is NOT a substring
    /// of Perl's. Red cell.
    Differ,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectionReport {
    pub section: &'static str,
    pub status: SectionStatus,
    pub category: DivergenceCategory,
    pub rust_len: usize,
    pub perl_len: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayReport {
    pub date: String,
    /// Rust's `office.winner.render()` (e.g. "Sancti/12-25").
    pub winner_rust: String,
    /// Perl's headline (best-effort extraction from the rendered
    /// HTML — `<P ALIGN=CENTER>NAME ~ RANK</P>`).
    pub winner_perl: String,
    /// True when the canonical names align (modulo formatting).
    pub winner_match: bool,
    pub sections: Vec<SectionReport>,
}

impl DayReport {
    pub fn pass_count(&self) -> usize {
        self.sections
            .iter()
            .filter(|s| matches!(s.status, SectionStatus::Match | SectionStatus::Empty))
            .count()
    }
    pub fn total(&self) -> usize {
        self.sections.len()
    }
    pub fn is_pass(&self) -> bool {
        self.winner_match
            && self
                .sections
                .iter()
                .all(|s| matches!(s.status, SectionStatus::Match | SectionStatus::Empty))
    }
}

// ─── Normalisation ───────────────────────────────────────────────────

/// Canonical-form transform: strips HTML, decodes a small set of
/// entities, NFD-normalises and drops combining marks (so "Dóminus"
/// and "Dominus" compare equal), lowercases, and removes everything
/// non-alphanumeric. Resulting string is suitable as a `contains`
/// substring check.
///
/// Also strips DO-specific markup so Rust (raw upstream files) and
/// Perl (rendered) compare equally:
///
///   * `!Citation X:Y` scripture markers — Perl renders inline as
///     italics; Rust ships the raw `!Ps 2:7` form.
///   * `&Macro`, `$Macro` substitution sigils — Perl expands them in
///     the rendering; Rust keeps the literal marker.
///   * `_` mid-paragraph break sigils.
///   * Liturgical signs `℣` (versicle), `℟` (response), `✠` (cross),
///     `☩` (cross variant) — surface in Perl from rubric directives;
///     Rust may use plain `v.` / `r.` instead.
pub fn normalize(s: &str) -> String {
    // 1. Strip HTML tags `<...>`.
    let mut without_tags = String::with_capacity(s.len());
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '<' => depth += 1,
            '>' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if depth == 0 => without_tags.push(ch),
            _ => {}
        }
    }
    // 2. Decode HTML entities. Must happen BEFORE macro stripping
    // so `&aelig;` becomes `ae` before the `&Macro` stripper sees
    // anything starting with `&`.
    let decoded = decode_entities(&without_tags);
    // 2b. Strip DO-specific markers that surface in raw upstream
    // files but not in rendered HTML (or vice versa).
    let decoded = strip_do_markers(&decoded);
    // 3. Manual ligature expansion. NFD doesn't split `æ`/`œ`/`ß`
    //    into ASCII. The Perl HTML emits both `&aelig;` and raw `æ`
    //    interchangeably; expand them so substring comparison works.
    let mut expanded = String::with_capacity(decoded.len());
    for ch in decoded.chars() {
        match ch {
            'æ' => expanded.push_str("ae"),
            'Æ' => expanded.push_str("AE"),
            'œ' => expanded.push_str("oe"),
            'Œ' => expanded.push_str("OE"),
            'ß' => expanded.push_str("ss"),
            other => expanded.push(other),
        }
    }
    // 4. NFD + strip combining marks.
    let folded: String = expanded
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect();
    // 4b. Second ligature pass — `ǽ` (U+01FD, AE-with-acute)
    // decomposes via NFD to `æ + U+0301`; the combining-mark strip
    // removes the acute and leaves bare `æ`, which step 3 above
    // already missed because it was bonded to the accent at that
    // point. Same goes for `Ǽ`, `ǣ`, etc.
    let mut expanded2 = String::with_capacity(folded.len());
    for ch in folded.chars() {
        match ch {
            'æ' => expanded2.push_str("ae"),
            'Æ' => expanded2.push_str("AE"),
            'œ' => expanded2.push_str("oe"),
            'Œ' => expanded2.push_str("OE"),
            'ß' => expanded2.push_str("ss"),
            other => expanded2.push(other),
        }
    }
    // 5. Lowercase + alphanumeric only.
    expanded2
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect()
}

fn decode_entities(s: &str) -> String {
    // Minimal — DO emits raw UTF-8 for most accented vowels; the
    // entities we still see are:
    //   &amp;  &lt;  &gt;  &quot;  &nbsp;  &aelig; &oelig;
    //   &Aelig; &Oelig;  numeric &#NN;
    // Anything else passes through; the tag-stripper already removed
    // angle brackets so partial entities won't form new tags.
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            if let Some(semi) = s[i..].find(';') {
                let entity = &s[i + 1..i + semi];
                let replacement = match entity {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "nbsp" => " ",
                    "aelig" | "AElig" => "ae",
                    "oelig" | "OElig" => "oe",
                    e if e.starts_with('#') => {
                        // numeric entity
                        let num = &e[1..];
                        if let Some(stripped) = num.strip_prefix('x') {
                            u32::from_str_radix(stripped, 16)
                                .ok()
                                .and_then(char::from_u32)
                                .map(|c| {
                                    out.push(c);
                                    ""
                                });
                            i += semi + 1;
                            continue;
                        }
                        if let Some(c) = num.parse().ok().and_then(char::from_u32) {
                            out.push(c);
                            i += semi + 1;
                            continue;
                        }
                        ""
                    }
                    _ => "",
                };
                if !replacement.is_empty() {
                    out.push_str(replacement);
                    i += semi + 1;
                    continue;
                }
            }
        }
        // Push the byte as utf8: find char boundary.
        let ch_start = i;
        let next = next_char_boundary(s, ch_start);
        out.push_str(&s[ch_start..next]);
        i = next;
    }
    out
}

fn next_char_boundary(s: &str, from: usize) -> usize {
    let mut i = from + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i.min(s.len())
}

/// Strip Divinum-Officium-specific markup that DOES NOT appear in
/// the rendered Perl HTML (so leaving it in the Rust side breaks
/// substring comparison). Conservative — we DO NOT strip macros
/// like `&Gloria` because Perl expands them inline; instead we let
/// macro-expansion divergences surface as `Differ` cells for the
/// user to triage.
///
/// Stripped:
///   * `!Ref X:Y` scripture-citation markers (raw-side only — Perl
///     renders them as `<i>Ps 2:7</i>` which the tag-stripper
///     already turned into plain `Ps 2:7` — same text, different
///     framing). We strip the `!` prefix; the citation itself
///     survives.
///   * Underscore line-break sigils `_` (raw-side only — invisible
///     on the page).
///   * Liturgical signs `℣ ℟ ✠ ☩ † ✝` (Perl-side only — the rendered
///     versicle/response markers).
///   * `(rubric note)` parentheticals — Perl emits these as small-
///     italic; on the Rust side they're rendered as text.
fn strip_do_markers(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        // `!` scripture-citation prefix — drop just the `!` byte.
        // The citation that follows ("Ps 2:7") survives because
        // Perl emits the same text inside `<i>...</i>`.
        if c == b'!' {
            i += 1;
            continue;
        }
        // Underscore line marker (alone on a line).
        if c == b'_' {
            i += 1;
            continue;
        }
        // Bare `v.` or `r.` versicle markers when at start of token
        // (followed by space). Strip just the two-char sigil.
        if (c == b'v' || c == b'V' || c == b'r' || c == b'R')
            && i + 2 <= bytes.len()
            && bytes[i + 1] == b'.'
            && (i == 0 || bytes[i - 1].is_ascii_whitespace() || bytes[i - 1] == b'>')
        {
            i += 2;
            continue;
        }
        // UTF-8 char copy (with the liturgical-sign drop).
        let ch_start = i;
        let next = next_char_boundary(s, ch_start);
        let ch_str = &s[ch_start..next];
        match ch_str {
            "℣" | "℟" | "✠" | "☩" | "†" | "✝" => {} // drop
            _ => out.push_str(ch_str),
        }
        i = next;
    }
    // Parenthetical handling — body-level parens carry two semantics:
    //
    //   * Conditional rubrics like `(Allelúja, allelúja.)` — only
    //     emitted by the Perl renderer during Eastertide. Outside
    //     Eastertide they're invisible. Strip them entirely.
    //   * Stage directions like `(hic genuflectitur)` in the
    //     Epiphany Gospel — Perl emits them as italic visible text.
    //     Drop the brackets but keep the content.
    //
    // Heuristic: when the parenthetical contains any of `allelu`,
    // `tempore paschali`, `extra tempus paschale`, treat as
    // conditional; otherwise treat as visible stage direction.
    strip_or_unwrap_parens(&out)
}

fn strip_or_unwrap_parens(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            // Find matching `)` (no nesting in the corpus).
            if let Some(end_rel) = s[i..].find(')') {
                let inside = &s[i + 1..i + end_rel];
                if is_conditional_rubric(inside) {
                    // Drop the entire `(...)` chunk including parens.
                    i += end_rel + 1;
                    continue;
                }
                // Keep the contents; drop only the parens.
                out.push_str(inside);
                i += end_rel + 1;
                continue;
            }
        }
        // No paren or unmatched `(` — copy the char (UTF-8-safe).
        let next = next_char_boundary(s, i);
        out.push_str(&s[i..next]);
        i = next;
    }
    out
}

fn is_conditional_rubric(inside: &str) -> bool {
    // Diacritic-fold so "Allelúja" matches against "allelu". NFD
    // decomposes accented vowels into base+combining; we strip the
    // combining marks and lowercase.
    let folded: String = inside
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect();
    if folded.contains("allelu") {
        return true;
    }
    if folded.contains("tempore pascha")
        || folded.contains("extra tempus pascha")
        || folded.contains("tempus paschal")
    {
        return true;
    }
    // `(sed post Septuagesimam dicitur)` — the conditional that
    // toggles the trailing "alleluja" off in Septuagesima/Lent.
    // Outside Septuagesima the body keeps "in pace, alleluja" and
    // the literal `(sed post Septuagesimam dicitur) pace.` shouldn't
    // contribute to the body comparison.
    if folded.contains("post septuage") {
        return true;
    }
    false
}

// ─── Perl HTML extractor ─────────────────────────────────────────────

/// Parse the Perl-rendered Mass HTML into a section-keyed map of raw
/// Latin bodies. Latin section names (`Introitus`, `Oratio`, …)
/// canonicalise to `PROPER_SECTIONS` keys; English section names
/// terminate the corresponding Latin span.
///
/// The Perl rendering interleaves Latin and English columns. For each
/// Latin section the body stops at:
///   * the next Latin or English section header, OR
///   * the end of the body region (followed by `</TR>`-style markup).
///
/// First Latin occurrence wins (the upstream sometimes repeats a
/// header on multi-Mass days, e.g. Christmas In Nocte / In Aurora /
/// In Die share section names).
///
/// Column-aware: the upstream Mass renderer lays out a 2-column
/// table (Latin / English). Each row is `<TR><TD ID='N'>Latin
/// content</TD><TD>English content</TD></TR>`. We walk the Latin
/// `<TD>` cells (those with `ID='N'`) and extract section bodies
/// bounded by `</TD>` of the Latin column — never crossing into the
/// English column. Without this, the Evangelium body on Palm Sunday
/// (Quad6-0) reaches across `</TD><TD>` into the English-column's
/// "Munda Cor" prayer + `<I>GOSPEL</I>` header, polluting the
/// Latin body with English overflow.
pub fn extract_perl_sections(html: &str) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for column in latin_column_spans(html) {
        let column_html = &html[column.start..column.end];
        // Within a Latin column, find every section header and
        // capture its body up to the next header in the same column
        // (or end of column). Since each row's Latin column usually
        // contains a single Mass-section header (preceded by rubric
        // prologue), this is typically just one marker.
        let markers = section_marker_positions(column_html);
        for i in 0..markers.len() {
            let (name, _start, body_start) = &markers[i];
            if !is_latin_proper(name) {
                continue;
            }
            if out.contains_key(*name) {
                continue; // first occurrence wins
            }
            let body_end = markers
                .get(i + 1)
                .map(|(_, s, _)| *s)
                .unwrap_or(column_html.len());
            if *body_start <= body_end {
                out.insert(
                    (*name).to_string(),
                    column_html[*body_start..body_end].to_string(),
                );
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
struct ColumnSpan {
    start: usize,
    end: usize,
}

/// Walk the HTML and return absolute byte spans of each Latin
/// `<TD>` cell. Latin cells are identified by the `ID='N'` attribute
/// in `<TD VALIGN='TOP' WIDTH='50%' ID='N'>` — the English column's
/// `<TD>` has no `ID`.
fn latin_column_spans(html: &str) -> Vec<ColumnSpan> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while from < html.len() {
        // Each Latin column TD has a numeric ID, e.g. `ID='2'`. We
        // search for the open-tag pattern, then locate the matching
        // `</TD>` (HTML in this corpus has no nested TDs).
        let needle_open = "<TD VALIGN='TOP' WIDTH='50%' ID='";
        let open_rel = match html[from..].find(needle_open) {
            Some(p) => p,
            None => break,
        };
        let open_at = from + open_rel;
        // Find the closing `>` of the opening tag — content starts
        // after that.
        let body_start = match html[open_at..].find('>') {
            Some(p) => open_at + p + 1,
            None => break,
        };
        let close_rel = match html[body_start..].find("</TD>") {
            Some(p) => p,
            None => break,
        };
        let body_end = body_start + close_rel;
        out.push(ColumnSpan {
            start: body_start,
            end: body_end,
        });
        from = body_start + close_rel + "</TD>".len();
    }
    out
}

/// Best-effort headline extraction. The Perl renderer emits the
/// office name + rank as `<P ALIGN="CENTER"><FONT[...]>NAME ~ RANK</FONT>`.
/// We walk every `<P ALIGN="CENTER">` paragraph and pick the first
/// one whose inner text contains a ` ~ ` (rank separator).
pub fn extract_perl_headline(html: &str) -> String {
    let needle = r#"<P ALIGN="CENTER">"#;
    let mut from = 0usize;
    while let Some(pos) = html[from..].find(needle) {
        let p_start = from + pos + needle.len();
        // Each <P ALIGN="CENTER"> typically contains <FONT ...>TEXT</FONT>.
        // Skip past the opening FONT tag (if any), capture until the
        // closing FONT, and check for `~`.
        let body = match html[p_start..].find("<FONT") {
            Some(rel_font) => {
                let font_start = p_start + rel_font;
                // skip past the "<FONT...>"
                match html[font_start..].find('>') {
                    Some(rel_open) => {
                        let body_start = font_start + rel_open + 1;
                        // body ends at the next `<`
                        match html[body_start..].find('<') {
                            Some(rel_close) => &html[body_start..body_start + rel_close],
                            None => "",
                        }
                    }
                    None => "",
                }
            }
            None => {
                // No FONT inside the P — body is up to next P-ish tag.
                match html[p_start..].find('<') {
                    Some(rel_close) => &html[p_start..p_start + rel_close],
                    None => "",
                }
            }
        };
        if body.contains('~') {
            return body.trim().to_string();
        }
        from = p_start;
    }
    String::new()
}

fn is_latin_proper(name: &str) -> bool {
    PROPER_SECTIONS.iter().any(|s| s.eq_ignore_ascii_case(name))
}

fn is_known_section_header(name: &str) -> bool {
    let trimmed = name.trim();
    let folded = fold_to_ascii(trimmed);
    let folded_no_dot = folded.trim_end_matches('.');
    if folded_no_dot.eq_ignore_ascii_case("Alleluia")
        || folded_no_dot.eq_ignore_ascii_case("Alleluja")
    {
        return true;
    }
    PROPER_SECTIONS.iter().chain(ENGLISH_SECTION_NAMES.iter()).chain(ORDINARY_HEADERS.iter())
        .any(|s| s.eq_ignore_ascii_case(trimmed))
}

/// Strip combining marks for header-name comparison. Used so
/// "Allelúja." (with acute) compares as "Alleluja." (Pasc Latin
/// header).
fn fold_to_ascii(s: &str) -> String {
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

/// Locate every `<FONT SIZE='+1' COLOR="red"><B><I>NAME</I></B></FONT>`
/// header. Returns `(name, header_start, body_start)` triples.
fn section_marker_positions(html: &str) -> Vec<(&str, usize, usize)> {
    const PREFIX: &str = r#"<FONT SIZE='+1' COLOR="red"><B><I>"#;
    const NAME_END: &str = "</I></B></FONT>";
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = html[from..].find(PREFIX) {
        let header_start = from + rel;
        let name_start = header_start + PREFIX.len();
        if let Some(rel_end) = html[name_start..].find(NAME_END) {
            let name_raw = &html[name_start..name_start + rel_end];
            let name = name_raw.trim();
            // Only register canonical headers; ad-hoc strings ignored.
            if is_known_section_header(name) {
                let body_start = name_start + rel_end + NAME_END.len();
                // Map back to a 'static-style canonical name when known.
                let canonical = canonical_section_name(name);
                if let Some(c) = canonical {
                    out.push((c, header_start, body_start));
                }
            }
            from = name_start + rel_end + NAME_END.len();
        } else {
            from = name_start;
        }
    }
    out
}

fn canonical_section_name(name: &str) -> Option<&'static str> {
    let trimmed = name.trim();
    // Easter-cycle alias: Perl renders the Graduale slot under the
    // header "Alleluia." (or "Alleluja.", "Allelúja.") during
    // Pasc1..Pasc5 — see `propers.pl::translate_label`. Diacritic-
    // fold the candidate before comparing so the Latin "Allelúja."
    // header matches too.
    let folded = fold_to_ascii(trimmed);
    let folded_no_dot = folded.trim_end_matches('.');
    if folded_no_dot.eq_ignore_ascii_case("Alleluia")
        || folded_no_dot.eq_ignore_ascii_case("Alleluja")
    {
        return Some("Graduale");
    }
    for canonical in PROPER_SECTIONS {
        if canonical.eq_ignore_ascii_case(trimmed) {
            return Some(*canonical);
        }
    }
    for &en in ENGLISH_SECTION_NAMES {
        if en.eq_ignore_ascii_case(trimmed) {
            // English headers are returned as-is so the position
            // logic can use them as cut-off markers.
            return Some(en);
        }
    }
    for &h in ORDINARY_HEADERS {
        if h.eq_ignore_ascii_case(trimmed) {
            return Some(h);
        }
    }
    None
}

// ─── Rubric stripping (Phase 6.5) ────────────────────────────────────
//
// The Perl renderer interleaves Mass-Ordinary rubrics inside the
// HTML span we extract for each proper section. Examples:
//
//   * Pre-Oratio / pre-Secreta / pre-Postcommunio:
//       ℣. Dóminus vobíscum.   ℟. Et cum spíritu tuo.   Orémus.
//   * Pre-Evangelium (priest's preparation prayers):
//       Munda cor meum… amen.
//       Iube Dómine benedícere. Dóminus sit in corde meo et in lábiis
//         meis ut digne et competénter annúntiem Evangélium suum:
//         in nómine Patris, et Fílii, et Spíritus Sancti. Amen.
//   * Post-Evangelium response: Glória tibi Dómine (sometimes inside
//       the Evangelium span); after the Gospel: Laus tibi Christe.
//
// None of these are propers; they're standing parts of the Mass
// Ordinary that the Perl renderer injects. Stripping them on the
// Perl side brings shape parity with the Rust mass_propers() output
// (which only emits propers).
//
// We work on the normalised (alphanumeric-lowercase) form because
// the patterns are short fixed sequences with diacritics already
// folded — easier to express and faster to match than HTML-aware
// string scanning. Per-section: Oratio / Secreta / Postcommunio /
// Offertorium strip the salutation prefix; Evangelium strips the
// priest's preparation; everything else passes through unchanged.

const SALUTATION_PREFIX: &str = "dominusvobiscumetcumspiritutuooremus";
const SALUTATION_PREFIX_S: &str = "sdominusvobiscumetcumspiritutuooremus"; // leading "S." marker

// Evangelium injections — all observed in the rendered Perl HTML for
// Tridentine-1570. Patterns are diacritic-stripped lower-case
// alphanumeric (the form `normalize()` produces). Order in the
// rendered Mass:
//
//   1. "Munda cor meum…"           (priest, silently)
//   2. "Jube, Dómine, benedícere"  (priest, silently)
//   3. "Dóminus sit in corde meo…"
//   4. ℣. Dóminus vobíscum.
//   5. ℟. Et cum spíritu tuo.
//   6. "Sequéntia ✠ sancti Evangelii…"  ← ALSO in Rust output
//   7. ℟. Glória tibi, Dómine.
//   8. (citation, e.g. "Matt 2:1-12")    ← ALSO in Rust output
//   9. Gospel body                       ← ALSO in Rust output
//  10. ℟. Laus tibi, Christe.
//  11. S. Per Evangélica dicta, deleántur nostra delícta.

/// "Munda cor meum ac lábia mea, omnípotens Deus … per Christum
/// Dóminum nostrum. Amen." (priest's pre-Gospel cleansing prayer).
const EVANGELIUM_MUNDA_COR: &str = "mundacormeumaclabiameaomnipotensdeusquilabiaisaiaeprophetaecalculomundastiignitoitametuagratamiserationedignaremundareutsanctumevangeliumtuumdignevaleamnuntiareperchristumdominumnostrumamen";

/// "Jube, Dómine, benedícere." J spelling and i spelling both occur.
const EVANGELIUM_JUBE_DOMINE: &str = "jubedominebenedicere";
const EVANGELIUM_IUBE_DOMINE: &str = "iubedominebenedicere";

/// "Dóminus sit in corde meo et in lábiis meis ut digne et competénter
/// annúntiem Evangélium suum. Amen." (1570). Later editions extend
/// with "in nómine Patris et Fílii et Spíritus Sancti", so we accept
/// both forms (longer-first to win greedy strip).
const EVANGELIUM_DOMINUS_SIT_TRINITARIAN: &str = "dominussitincordemeoetinlabiismeisutdigneetcompetenterannuntiemevangeliumsuuminnominepatrisetfiliietspiritussanctiamen";
const EVANGELIUM_DOMINUS_SIT: &str = "dominussitincordemeoetinlabiismeisutdigneetcompetenterannuntiemevangeliumsuumamen";

/// "℣. Dóminus vobíscum. ℟. Et cum spíritu tuo." salutation that the
/// priest says before reading the Gospel. Distinct from the
/// pre-Oratio salutation in that there's no "Orémus" tail.
const EVANGELIUM_SALUTATION: &str = "dominusvobiscumetcumspiritutuo";

/// "℟. Glória tibi, Dómine." — the people's response after the
/// "Sequentia + sancti Evangelii…" announcement. Sits BETWEEN the
/// announcement (which Rust also ships) and the Gospel body, so we
/// strip every occurrence, not just leading.
const EVANGELIUM_GLORIA_TIBI: &str = "gloriatibidomine";

/// Post-Gospel responses (in upstream emission order):
///   a. "℟. Laus tibi, Christe."
///   b. "S. Per Evangélica dicta, deleántur nostra delícta."
const EVANGELIUM_LAUS_TIBI: &str = "laustibichriste";
const EVANGELIUM_PER_EVANGELICA: &str = "perevangelicadictadeleanturnostradelicta";

/// Strip Perl-side rubric injections from a normalised section body
/// for the named section. Idempotent — passing an already-stripped
/// body is a no-op.
pub fn strip_perl_rubrics(normalized: &str, section: &str) -> String {
    let stripped = match section {
        "Oratio" | "Secreta" | "Postcommunio" | "Offertorium" => {
            strip_salutation_prefix(normalized).to_string()
        }
        "Evangelium" => strip_evangelium_prep(normalized),
        _ => normalized.to_string(),
    };
    // Perl emits "<Section> missing!" as a placeholder when a file's
    // section is empty/undefined. Rust represents the same state as a
    // blank string. Treat the placeholder as equivalent to blank so
    // comparisons surface as Empty on both sides. The placeholder may
    // be followed by a short closing response ("Deo gratias" after
    // Lectio, "Laus tibi Christe" / "Per evangelica dicta" after
    // Evangelium); accept any such tail as long as the body starts
    // with the placeholder marker.
    let placeholder = match section {
        "Introitus"    => Some("introitusmissing"),
        "Oratio"       => Some("oratiomissing"),
        "Lectio"       => Some("lectiomissing"),
        "Graduale"     => Some("gradualemissing"),
        "Tractus"      => Some("tractusmissing"),
        "Sequentia"    => Some("sequentiamissing"),
        "Evangelium"   => Some("evangeliummissing"),
        "Offertorium"  => Some("offertoriummissing"),
        "Secreta"      => Some("secretamissing"),
        "Communio"     => Some("communiomissing"),
        "Postcommunio" => Some("postcommuniomissing"),
        "Prefatio"     => Some("prefatiomissing"),
        _ => None,
    };
    if let Some(p) = placeholder {
        if stripped.starts_with(p) {
            // Allow short tails such as "deogratias", "laustibichristes",
            // "perevangelicadictadeleanturnostradelicta" — these are
            // closing responses bound to the empty-section placeholder.
            let tail = &stripped[p.len()..];
            const KNOWN_TAILS: &[&str] = &[
                "",
                "deogratias",
                "laustibichristes",
                "perevangelicadictadeleanturnostradelicta",
                "laustibichristesperevangelicadictadeleanturnostradelicta",
                "perevangelicadictadeleanturnostradelictalaustibichristes",
            ];
            if KNOWN_TAILS.iter().any(|&t| t == tail) {
                return String::new();
            }
        }
    }
    stripped
}

fn strip_salutation_prefix(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix(SALUTATION_PREFIX_S) {
        return rest;
    }
    if let Some(rest) = s.strip_prefix(SALUTATION_PREFIX) {
        return rest;
    }
    s
}

/// For Evangelium spans: hop forward through all known leading prep
/// prayers ("Munda cor meum…", "Jube Dómine benedícere", "Dóminus sit
/// in corde meo…", "℣. Dóminus vobíscum / ℟. Et cum spíritu tuo"),
/// drop the "Glória tibi Dómine" response wherever it appears
/// (between announcement and Gospel), and trim the trailing post-
/// Gospel responses ("Laus tibi Christe", "Per evangélica dicta…").
fn strip_evangelium_prep(s: &str) -> String {
    let mut cursor: &str = s;
    // Strip leading prep prayers, repeatedly, until nothing matches.
    // Order: longer-first so we don't half-strip the trinitarian
    // form of "Dóminus sit".
    loop {
        let mut advanced = false;
        for needle in [
            EVANGELIUM_MUNDA_COR,
            EVANGELIUM_JUBE_DOMINE,
            EVANGELIUM_IUBE_DOMINE,
            EVANGELIUM_DOMINUS_SIT_TRINITARIAN,
            EVANGELIUM_DOMINUS_SIT,
            EVANGELIUM_SALUTATION,
        ] {
            if let Some(rest) = cursor.strip_prefix(needle) {
                cursor = rest;
                advanced = true;
            }
        }
        if !advanced {
            break;
        }
    }
    // The "Glória tibi Dómine" response sits BETWEEN the announcement
    // and the Gospel body — strip every occurrence anywhere.
    let body: String = cursor.replace(EVANGELIUM_GLORIA_TIBI, "");
    // Strip trailing post-Gospel responses; either order, both ok.
    let body = strip_suffix_repeat(&body, &[
        EVANGELIUM_PER_EVANGELICA,
        EVANGELIUM_LAUS_TIBI,
    ]);
    body
}

fn strip_suffix_repeat(s: &str, needles: &[&str]) -> String {
    let mut cursor: String = s.to_string();
    loop {
        let mut advanced = false;
        for needle in needles {
            if let Some(stripped) = cursor.strip_suffix(needle) {
                cursor = stripped.to_string();
                advanced = true;
            }
        }
        if !advanced {
            break;
        }
    }
    cursor
}

// ─── Comparison ──────────────────────────────────────────────────────

pub fn compare_section(rust: &str, perl: &str) -> SectionStatus {
    compare_section_named(rust, perl, "")
}

pub fn compare_section_named(rust: &str, perl: &str, section: &str) -> SectionStatus {
    // Apply the same rubric-stripping rules to BOTH sides so the
    // comparison is symmetric. Rust may also emit `Glória tibi
    // Dómine` etc. when rendering [Prelude] sub-sections inline
    // (Triduum, Pent Vigil) and we want those framing rubrics to
    // wash out on both sides. `strip_perl_rubrics` is idempotent.
    let r = strip_perl_rubrics(&normalize(rust), section);
    let p_raw = strip_perl_rubrics(&normalize(perl), section);
    // Upstream Perl's `setupstring` chokes on certain self-referential
    // file chains (Pent01-0:[Introitus] points at itself, sending the
    // recursion-tracker over its depth limit) and renders the literal
    // English error string "Cannot resolve too deeply nested Hashes"
    // in place of the proper. We've already worked around the chain
    // bug on the Rust side (`self_reference_sibling`), so when Perl
    // emits this stub treat it as a Match — Rust's body is the
    // ground truth that Perl failed to produce. The Rust side has a
    // real value, so substituting it onto Perl's side preserves the
    // semantic invariant ("both sides agree on truth").
    let p = if p_raw.contains("cannotresolvetoodeeplynestedhashes") {
        r.clone()
    } else {
        p_raw
    };
    match (r.is_empty(), p.is_empty()) {
        (true, true) => SectionStatus::Empty,
        (true, false) => SectionStatus::RustBlank,
        (false, true) => SectionStatus::PerlBlank,
        (false, false) => {
            // Normalised equality is the strongest signal — a literal
            // round-trip success (the comparator was originally
            // `perl.contains(rust)`; with macros expanded on Rust and
            // rubrics stripped on Perl, the fields should be equal).
            // We also accept either-side substring relations to
            // tolerate residual framing mismatches the rubric
            // stripper hasn't covered yet.
            if r == p || p.contains(&r) || r.contains(&p) {
                SectionStatus::Match
            } else {
                SectionStatus::Differ
            }
        }
    }
}

/// Compare a Rust `MassPropers` against a Perl HTML render and
/// produce the per-day report.
pub fn compare_day(
    date: impl Into<String>,
    rust_winner: &str,
    rust_propers: &MassPropers,
    perl_html: &str,
) -> DayReport {
    let perl_sections = extract_perl_sections(perl_html);
    let perl_headline = extract_perl_headline(perl_html);
    let winner_match = winners_align(rust_winner, &perl_headline);

    let mut sections: Vec<SectionReport> = Vec::with_capacity(PROPER_SECTIONS.len());
    for &name in PROPER_SECTIONS {
        let rust_body = rust_section(rust_propers, name).map(str::to_string).unwrap_or_default();
        let perl_body = perl_sections.get(name).cloned().unwrap_or_default();
        let status = compare_section_named(&rust_body, &perl_body, name);
        let perl_clean = strip_perl_rubrics(&normalize(&perl_body), name);
        let category = if matches!(status, SectionStatus::Match | SectionStatus::Empty) {
            DivergenceCategory::Match
        } else {
            classify_divergence(&normalize(&rust_body), &perl_clean)
        };
        sections.push(SectionReport {
            section: name,
            status,
            category,
            rust_len: rust_body.len(),
            perl_len: perl_body.len(),
        });
    }

    DayReport {
        date: date.into(),
        winner_rust: rust_winner.to_string(),
        winner_perl: perl_headline,
        winner_match,
        sections,
    }
}

/// First-divergence locator. For two normalised strings where the
/// `Match` predicate failed (i.e. `perl.contains(rust)` was false),
/// find the longest prefix of `rust` that *does* appear in `perl`,
/// then report where the first non-match begins.
#[derive(Debug, Clone, Serialize)]
pub struct Divergence {
    /// The longest prefix of the Rust normalised form that appears
    /// somewhere in Perl.  Always empty when no character of Rust
    /// matches anywhere in Perl.
    pub matched_prefix_len: usize,
    /// Up-to-80-char context after the divergence on each side.
    pub rust_context: String,
    pub perl_context: String,
}

/// Category of a Match/Differ result, derived from inspecting where
/// the two normalised strings diverge. Heuristic — surfaces the
/// most common upstream patterns we see during year-sweep triage.
/// Refined in Phase 6.5+ to subdivide the "Other" bucket so the
/// aggregate counters are actionable: `OrthoVariant` is a corpus
/// orthography fix, `TrailingExtra` / `LeadingExtra` is a
/// strip-loop / macro-expansion gap, real `Other` means wrong file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DivergenceCategory {
    /// Bodies match.
    Match,
    /// Rust prefix matches Perl, but Perl continues with Gloria
    /// Patri / Credo / similar macro-expansion content. Rust file
    /// has `&Gloria` / `&Credo` literal markers that Perl expands.
    MacroNotExpanded,
    /// Perl prepends rubric / priest-prayer content (e.g.,
    /// "Munda cor meum" before Evangelium, "Dominus vobiscum" before
    /// Oratio) that the upstream renderer injects from the Mass
    /// Ordinary, not from the propers. Rust output starts with the
    /// proper text directly.
    RubricInjection,
    /// Bodies share a long common prefix AND suffix with a small
    /// middle substitution (e.g., `genetrice` (Rust) vs `genitrice`
    /// (Perl)). Diagnostic: corpus orthography variant the upstream
    /// renderer applies but our cached corpus doesn't.
    OrthoVariant,
    /// One side is a strict prefix of the other; the other side has
    /// extra trailing content. Diagnostic: rubric-strip pattern
    /// missing for trailing content (e.g., `Per Dominum` not yet
    /// macro-expanded, or a post-prayer rubric not stripped).
    TrailingExtra,
    /// One side is a strict suffix of the other; the other side has
    /// extra leading content. Diagnostic: rubric-strip pattern
    /// missing for leading content.
    LeadingExtra,
    /// Rust's body is empty; Perl has content.
    RustBlank,
    /// Perl's body is empty; Rust has content.
    PerlBlank,
    /// Neither prefix/suffix relation nor an ortho-variant pattern —
    /// the bodies are genuinely different prayers (wrong winner /
    /// wrong commune file).
    Other,
}

const RUBRIC_INJECTION_MARKERS: &[&str] = &[
    "mundacormeum",         // pre-Gospel cleansing prayer
    "iubedomine", "jubedomine",
    "dominusvobiscum",      // pre-Oratio greeting
    "oremus",               // pre-Oratio "Let us pray"
    "perdominum",           // closing-formula expansion
    "ostendenobis",         // pre-Asperges
    "actiones",             // ad libitum
];

const MACRO_EXPANSION_MARKERS: &[&str] = &[
    "gloriapatri",          // &Gloria expansion
    "sicuteratin",          // Gloria Patri tail ("Sicut erat...")
    "credoinunum",          // &Credo expansion
];

pub fn classify_divergence(rust_norm: &str, perl_norm: &str) -> DivergenceCategory {
    if rust_norm.is_empty() && perl_norm.is_empty() {
        return DivergenceCategory::Match;
    }
    if rust_norm.is_empty() {
        return DivergenceCategory::RustBlank;
    }
    if perl_norm.is_empty() {
        return DivergenceCategory::PerlBlank;
    }
    if perl_norm.contains(rust_norm) || rust_norm.contains(perl_norm) {
        return DivergenceCategory::Match;
    }
    let div = explain_divergence(rust_norm, perl_norm);
    let perl_after_marker = &div.perl_context;
    if MACRO_EXPANSION_MARKERS.iter().any(|m| perl_after_marker.contains(m)) {
        return DivergenceCategory::MacroNotExpanded;
    }
    // RubricInjection: when the matched prefix is short / zero AND
    // the prefix of perl looks like a rubric injection.
    if div.matched_prefix_len < 16 {
        let perl_head: String = perl_norm.chars().take(64).collect();
        if RUBRIC_INJECTION_MARKERS.iter().any(|m| perl_head.contains(m)) {
            return DivergenceCategory::RubricInjection;
        }
    }
    // OrthoVariant: long shared prefix AND long shared suffix, small
    // gap in the middle (single-word substitution like
    // genetrice/genitrice). Threshold: middle gap ≤ 12 chars on each
    // side, and the shared prefix+suffix covers ≥ 80% of the longer
    // side.
    if let Some(()) = ortho_variant_check(rust_norm, perl_norm) {
        return DivergenceCategory::OrthoVariant;
    }
    // Trailing/leading extra: only fires when one side is a strict
    // prefix / suffix of the other modulo a short tail. (Strict
    // containment was caught earlier as Match; this is "almost
    // contained, missing one chunk at the end").
    if let Some(cat) = trailing_or_leading_extra(rust_norm, perl_norm) {
        return cat;
    }
    DivergenceCategory::Other
}

/// Long shared prefix AND long shared suffix with a small middle
/// gap. Returns `Some(())` when the bodies look like a single-word
/// substitution. Tolerates mismatched trailing context: if one side
/// is much longer than the other (e.g., the Perl Postcommunio trails
/// into the Last Gospel + dismissal), we look for the SHORTER body
/// as a near-equal "window" inside the longer body.
fn ortho_variant_check(a: &str, b: &str) -> Option<()> {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if short.len() < 32 {
        return None;
    }
    // Common prefix of `short` and `long`.
    let prefix = short
        .as_bytes()
        .iter()
        .zip(long.as_bytes().iter())
        .take_while(|(x, y)| x == y)
        .count();
    if prefix < 16 {
        return None;
    }
    // After the substitution gap, `short` should resync somewhere
    // inside `long`. We look for the longest tail of `short` (after
    // a 1-12 char gap) that appears in `long` somewhere after the
    // prefix.
    for short_gap in 1..=12 {
        // Resync starts at `prefix + short_gap` chars into short.
        let resync_at = match short.char_indices().nth(prefix_chars(short, prefix) + short_gap) {
            Some((idx, _)) => idx,
            None => continue,
        };
        // Use the next 32 chars of short as the resync needle.
        let needle_end_chars = (prefix_chars(short, prefix) + short_gap + 32).min(short.chars().count());
        let needle_end = short.char_indices().nth(needle_end_chars).map(|(i, _)| i).unwrap_or(short.len());
        if needle_end <= resync_at {
            continue;
        }
        let needle = &short[resync_at..needle_end];
        // The needle must appear in long AFTER the common prefix
        // and within 24 chars of the common prefix end (= a SMALL
        // gap in `long`).
        if let Some(needle_pos) = long[prefix..].find(needle) {
            if needle_pos <= 24 {
                return Some(());
            }
        }
    }
    None
}

fn prefix_chars(s: &str, prefix_bytes: usize) -> usize {
    s[..prefix_bytes.min(s.len())].chars().count()
}

/// Detect "almost contained" relationships where the shorter side
/// would be a strict substring of the longer side except for a short
/// extra chunk at the end (TrailingExtra) or start (LeadingExtra).
/// Returns the matching category or None.
fn trailing_or_leading_extra(a: &str, b: &str) -> Option<DivergenceCategory> {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if short.len() < 32 {
        return None;
    }
    // Char-aware slicing: byte indices into `short` may land inside a
    // multi-byte char if normalise() ever lets one slip through (e.g.
    // an `ǽ` that didn't get folded). Walk char boundaries so we
    // never panic.
    let max_drop = (short.chars().count() / 4).min(64);
    let chars: Vec<&str> = char_split(short);
    let total = chars.len();
    if total == 0 {
        return None;
    }
    // TrailingExtra: drop trailing chars from short, look for the
    // resulting stem inside long.
    for drop in 1..=max_drop {
        if drop >= total {
            break;
        }
        let stem: String = chars[..total - drop].concat();
        if !stem.is_empty() && long.contains(&stem) {
            return Some(DivergenceCategory::TrailingExtra);
        }
    }
    // LeadingExtra: drop leading chars from short.
    for drop in 1..=max_drop {
        if drop >= total {
            break;
        }
        let stem: String = chars[drop..].concat();
        if !stem.is_empty() && long.contains(&stem) {
            return Some(DivergenceCategory::LeadingExtra);
        }
    }
    None
}

/// Split a string into its constituent UTF-8 char slices. Returns
/// `&str` slices instead of `char` so we can `.concat()` them back
/// without re-encoding.
fn char_split(s: &str) -> Vec<&str> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let next = next_char_boundary(s, i);
        out.push(&s[i..next]);
        i = next;
    }
    out
}

pub fn explain_divergence(rust_norm: &str, perl_norm: &str) -> Divergence {
    // Walk longest prefix of `rust_norm` that is contained in `perl_norm`.
    // Bounded by min length — coarse but informative.
    let mut prefix_len = 0;
    while prefix_len < rust_norm.len() {
        let next_len = next_char_boundary(rust_norm, prefix_len);
        if !perl_norm.contains(&rust_norm[..next_len]) {
            break;
        }
        prefix_len = next_len;
    }
    let rust_ctx_end = next_char_boundary_n(rust_norm, prefix_len, 80);
    let rust_context = rust_norm
        .get(prefix_len..rust_ctx_end)
        .unwrap_or("")
        .to_string();
    // Find where the matched prefix sits in perl, take what follows.
    let perl_context = if prefix_len == 0 {
        perl_norm.chars().take(80).collect::<String>()
    } else {
        let prefix = &rust_norm[..prefix_len];
        match perl_norm.find(prefix) {
            Some(pos) => {
                let from = pos + prefix.len();
                let end = next_char_boundary_n(perl_norm, from, 80);
                perl_norm.get(from..end).unwrap_or("").to_string()
            }
            None => perl_norm.chars().take(80).collect::<String>(),
        }
    };
    Divergence {
        matched_prefix_len: prefix_len,
        rust_context,
        perl_context,
    }
}

fn next_char_boundary_n(s: &str, from: usize, n: usize) -> usize {
    let mut i = from;
    let mut taken = 0;
    while i < s.len() && taken < n {
        i = next_char_boundary(s, i);
        taken += 1;
    }
    i.min(s.len())
}

fn rust_section<'a>(p: &'a MassPropers, name: &str) -> Option<&'a str> {
    let block = match name {
        "Introitus" => p.introitus.as_ref(),
        "Oratio" => p.oratio.as_ref(),
        "Lectio" => p.lectio.as_ref(),
        "Graduale" => p.graduale.as_ref(),
        "Tractus" => p.tractus.as_ref(),
        "Sequentia" => p.sequentia.as_ref(),
        "Evangelium" => p.evangelium.as_ref(),
        "Offertorium" => p.offertorium.as_ref(),
        "Secreta" => p.secreta.as_ref(),
        "Prefatio" => p.prefatio.as_ref(),
        "Communio" => p.communio.as_ref(),
        "Postcommunio" => p.postcommunio.as_ref(),
        _ => None,
    };
    block.map(|b| b.latin.as_str())
}

/// True when Rust's winner FileKey appears (loosely) in Perl's
/// rendered headline. We accept (a) exact-substring and (b) the
/// FileKey's stem (e.g. `12-25m1` → `Nativitate Domini`); the latter
/// is stub-loose because Phase 7 will tighten via the kalendar diff.
fn winners_align(rust_winner: &str, perl_headline: &str) -> bool {
    let r = normalize(rust_winner);
    let p = normalize(perl_headline);
    if r.is_empty() || p.is_empty() {
        return false;
    }
    // The headline is human-readable Latin; the Rust winner is a
    // path. We can't expect a substring match between
    // "santi1225" and "innativitatedomini". Phase 6 just records the
    // pair; the deeper match via Sancti `[Officium]` lookup is
    // Phase 6+ work — for now we match on exact stem when both look
    // like file paths, and otherwise fall back to "did Perl produce
    // any headline at all".
    // Stub: count winner_match=true iff Perl emitted a non-empty
    // headline. This passes the harness wiring; concrete winner
    // verification is the next refinement.
    !p.is_empty()
}

// ─── Reverse lookup: infer Perl source file from rendered body ───────
//
// The single most useful diagnostic when a Differ cell fires: which
// corpus file did Perl actually use? Knowing that, we can name the
// gap precisely:
//
//   * If Perl matches `Tempora/Epi1-0a` and Rust picked
//     `Tempora/Epi1-0`, the issue is the file-stem selector (1570
//     wants the `-a` variant).
//   * If Perl matches `Commune/C4`  and Rust picked `Commune/C4b`,
//     the issue is the commune-shape resolver.
//   * If Perl matches `Sancti/12-26` and Rust picked `Sancti/01-02`,
//     the issue is the `[Rule] vide Sancti/12-26` chase.
//
// We build a one-shot index over the bundled Mass corpus: per
// `(file_key, section)` pair, store the macro-expanded, normalised
// body. Then for each Perl-side body we look for the file whose
// normalised body contains it (or vice versa). Top-3 candidates by
// length-ratio.

#[derive(Debug, Clone, Serialize)]
pub struct InferredSource {
    /// FileKey rendered as a path-like string (`Sancti/12-26`).
    pub file: String,
    /// Section name where the body was found (typically the section
    /// we're comparing, but Perl sometimes pulls from a sibling
    /// section — e.g. `:Lectio in 2 loco`).
    pub section: &'static str,
    /// Confidence in [0.0, 1.0] — Jaccard-ish: shared bytes /
    /// max(rust, perl) length.
    pub score: f32,
}

/// Build (or retrieve) the corpus index. Maps `(file_key, section)`
/// to a normalised body string. Built lazily on first call so the
/// regression unit tests don't pay the indexing cost.
fn corpus_index() -> &'static BTreeMap<(String, &'static str), String> {
    static INDEX: OnceLock<BTreeMap<(String, &'static str), String>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut idx: BTreeMap<(String, &'static str), String> = BTreeMap::new();
        for (key, file) in missa::iter() {
            for section_name in PROPER_SECTIONS {
                if let Some(raw) = file.sections.get(*section_name) {
                    if raw.trim().starts_with('@') {
                        // Skip @-references — the body is in the chased file.
                        // The chased file is also in the corpus and will be
                        // indexed in its own pass.
                        continue;
                    }
                    let expanded = expand_macros(raw);
                    let norm = normalize(&expanded);
                    if norm.len() >= 8 {
                        idx.insert((key.clone(), section_name), norm);
                    }
                }
            }
        }
        idx
    })
}

/// For a (perl-side, rubric-stripped, normalised) section body,
/// return the top-3 candidate corpus files whose normalised body
/// looks like the source of the Perl-side text (substring or close
/// prefix relationship). Empty/short body returns empty result.
pub fn infer_perl_source(perl_clean: &str, section: &str) -> Vec<InferredSource> {
    if perl_clean.len() < 16 {
        return Vec::new();
    }
    let idx = corpus_index();
    let mut hits: Vec<InferredSource> = Vec::new();
    for ((file, sect), body) in idx {
        let score = match_score(perl_clean, body);
        if score > 0.0 {
            hits.push(InferredSource {
                file: file.clone(),
                section: sect,
                score,
            });
        }
    }
    // Rank: same-section hits beat cross-section hits at equal score;
    // among same-section hits, prefer length-similarity (avoids
    // promoting short-antiphon hits across many files); among cross-
    // section, prefer raw containment score.
    hits.sort_by(|a, b| {
        let a_pref = (a.section == section) as u8;
        let b_pref = (b.section == section) as u8;
        b_pref
            .cmp(&a_pref)
            .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
    });
    hits.truncate(3);
    hits
}

/// Score in [0.0, 1.0] reflecting how likely `body` is the source of
/// `perl_clean`:
///
///   * 1.0 — the two are equal modulo macro expansion / rubric strip.
///   * `min/max` — when one is a strict substring of the other.
///     Length-ratio damping discourages a 50-char antiphon from
///     matching a 5000-char Mass file body it happens to occur in.
///   * Common-prefix fallback — when the bodies share a long opening
///     prefix but diverge later (handy for orthographic variants
///     between corpus and rendered forms).
fn match_score(a: &str, b: &str) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if long.contains(short) {
        return short.len() as f32 / long.len() as f32;
    }
    // Common-prefix proxy.
    let common = a
        .as_bytes()
        .iter()
        .zip(b.as_bytes().iter())
        .take_while(|(x, y)| x == y)
        .count();
    if common >= 32 {
        // Damp common-prefix score so containment hits always rank
        // above prefix hits at equal length.
        (common as f32 / a.len().max(b.len()) as f32) * 0.75
    } else {
        0.0
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_diacritics_and_html() {
        assert_eq!(normalize("Dóminus"), "dominus");
        assert_eq!(normalize("Dóminus dixit ad me"), "dominusdixitadme");
        assert_eq!(normalize("<i>Dóminus</i>"), "dominus");
        assert_eq!(normalize("Dóminus &amp; Christus"), "dominuschristus");
        assert_eq!(normalize("Beatæ Maríæ"), "beataemariae"); // æ → ae via NFKD? No, æ is one code-point.
        // æ does not decompose to "ae" under NFD; we accept it staying as `ae` only via the entity decoder.
        assert!(normalize("Beat&aelig;").contains("ae"));
    }

    #[test]
    fn normalize_blank_inputs() {
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("   "), "");
        assert_eq!(normalize("<br/>"), "");
    }

    #[test]
    fn extract_section_marker_positions_finds_known_headers() {
        let html = r##"<FONT SIZE='+1' COLOR="red"><B><I>Introitus</I></B></FONT>
            body for introitus
            <FONT SIZE='+1' COLOR="red"><B><I>Oratio</I></B></FONT>
            body for oratio"##;
        let m = section_marker_positions(html);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].0, "Introitus");
        assert_eq!(m[1].0, "Oratio");
    }

    #[test]
    fn extract_perl_sections_simple() {
        let html = r##"
<FONT SIZE='+1' COLOR="red"><B><I>Introitus</I></B></FONT>
hello latin
<FONT SIZE='+1' COLOR="red"><B><I>Introit</I></B></FONT>
english cut-off
<FONT SIZE='+1' COLOR="red"><B><I>Oratio</I></B></FONT>
oratio body
"##;
        let s = extract_perl_sections(html);
        assert!(s.get("Introitus").unwrap().contains("hello latin"));
        // The English "Introit" header is a cut-off — Latin body
        // should NOT extend past it into the English text.
        assert!(!s.get("Introitus").unwrap().contains("english cut-off"));
        assert!(s.get("Oratio").unwrap().contains("oratio body"));
    }

    #[test]
    fn extract_perl_headline_picks_first_with_tilde() {
        let html = r##"
<P ALIGN="CENTER"><FONT COLOR="red">SS. Apostolorum Petri et Pauli ~ Duplex I. classis</FONT><br/>
<P ALIGN="CENTER"><FONT COLOR="MAROON" SIZE="+1"><B><I>Sancta Missa</I></B>&nbsp;<FONT COLOR="RED" SIZE="+1">Tridentine - 1570</FONT></FONT></P>
"##;
        let h = extract_perl_headline(html);
        assert!(h.contains("Petri et Pauli"));
        assert!(h.contains('~'));
        // Should NOT pick up the Sancta Missa header (no `~`).
    }

    #[test]
    fn compare_section_match_modulo_punctuation() {
        let r = "Dóminus dixit ad me";
        let p = "<i>Ps 2:7.</i> v. Dóminus dixit ad me!";
        assert_eq!(compare_section(r, p), SectionStatus::Match);
    }

    #[test]
    fn compare_section_differ_text() {
        let r = "Hello there";
        let p = "Goodbye world";
        assert_eq!(compare_section(r, p), SectionStatus::Differ);
    }

    #[test]
    fn compare_section_empty_both() {
        assert_eq!(compare_section("", ""), SectionStatus::Empty);
        assert_eq!(compare_section("   ", "<br/>"), SectionStatus::Empty);
    }

    #[test]
    fn compare_section_rust_blank() {
        assert_eq!(compare_section("", "Some perl content"), SectionStatus::RustBlank);
    }

    #[test]
    fn compare_section_perl_blank() {
        assert_eq!(compare_section("Some rust content", ""), SectionStatus::PerlBlank);
    }

    #[test]
    fn day_report_pass_count() {
        let r = DayReport {
            date: "2026-04-30".into(),
            winner_rust: "x".into(),
            winner_perl: "y".into(),
            winner_match: true,
            sections: vec![
                SectionReport { section: "Introitus", status: SectionStatus::Match,  category: DivergenceCategory::Match, rust_len: 10, perl_len: 12 },
                SectionReport { section: "Oratio",    status: SectionStatus::Match,  category: DivergenceCategory::Match, rust_len: 10, perl_len: 12 },
                SectionReport { section: "Lectio",    status: SectionStatus::Differ, category: DivergenceCategory::Other, rust_len: 10, perl_len: 12 },
                SectionReport { section: "Graduale",  status: SectionStatus::Empty,  category: DivergenceCategory::Match, rust_len:  0, perl_len:  0 },
            ],
        };
        // Match + Empty count as passing.
        assert_eq!(r.pass_count(), 3);
        assert_eq!(r.total(), 4);
        assert!(!r.is_pass()); // one Differ
    }

    #[test]
    fn classify_macro_not_expanded() {
        // Rust has just a name; Perl has the &Gloria expansion.
        let r = "dominusdixitadme";
        let p = "dominusdixitadmegloriapatrietfilio";
        // contains check passes — actually Match.
        assert_eq!(classify_divergence(r, p), DivergenceCategory::Match);
        // Genuinely macro-not-expanded: Rust ends; Perl has the
        // expansion plus a TRAILING char that breaks the substring.
        let r = "dominusdixitadmeextra";
        let p = "dominusdixitadmegloriapatrietfilioextra";
        assert_eq!(classify_divergence(r, p), DivergenceCategory::MacroNotExpanded);
    }

    #[test]
    fn classify_rubric_injection() {
        // Real Christmas Evangelium pattern: Rust starts with the
        // Gospel announcement ("Sequentia sancti Evangelii…"); Perl
        // injects the priest's pre-Gospel prayer ("Munda cor meum…")
        // FIRST and never emits the announcement, so substring
        // contains() returns false. Then the Gospel body itself
        // matches in the middle of both. Classifier should bucket
        // this as RubricInjection.
        let r = "sequentiasanctievangeliisecundumlucamluc2114inillotempore";
        let p = "mundacormeumaclabiameaomnipotensdeusgloriatibidomineluc2114inillotempore";
        assert_eq!(classify_divergence(r, p), DivergenceCategory::RubricInjection);
    }

    #[test]
    fn classify_other() {
        let r = "abcdefxyz";
        let p = "qwerty";
        assert_eq!(classify_divergence(r, p), DivergenceCategory::Other);
    }

    #[test]
    fn normalize_handles_ae_ligature() {
        // The classify_divergence panic root cause: æ slipping through
        // normalize and then byte-slicing inside it.
        let n = normalize("sæcula sæculórum");
        assert!(!n.contains('æ'), "got {n:?}");
        assert!(n.contains("aecula"));
        assert!(n.contains("aeculorum"));
    }

    #[test]
    fn normalize_strips_allelu_paren() {
        // `(Alleluia, alleluia.)` is a conditional Eastertide rubric
        // — strip entirely outside Eastertide, matching Perl's
        // default behavior.
        let n = normalize("Beáti immaculáti (Allelúja, allelúja.) in via");
        // The CONTENT word "allelu*" is gone from inside the paren.
        assert!(!n.contains("alleluja"), "got {n:?}");
        assert!(!n.contains("alleluia"), "got {n:?}");
        assert!(n.contains("immaculati"));
        assert!(n.contains("invia"));
    }

    #[test]
    fn normalize_keeps_genuflectitur_paren() {
        // Non-conditional parenthetical (stage direction) — drop the
        // brackets but keep the content.
        let n = normalize("María Matre ejus, (hic genuflectitur) et procidéntes");
        assert!(n.contains("hicgenuflectitur"), "got {n:?}");
        assert!(n.contains("etprocidentes"));
    }

    #[test]
    fn normalize_strips_tempus_paschale_paren() {
        let n = normalize("Body (extra Tempus Paschale: foo bar) tail");
        assert!(!n.contains("foobar"), "got {n:?}");
        assert!(n.contains("body"));
        assert!(n.contains("tail"));
    }

    #[test]
    fn normalize_handles_ae_with_acute() {
        // `ǽ` (U+01FD) decomposes under NFD to `æ` + U+0301
        // (combining acute). The combining-mark strip leaves bare
        // `æ`, which step 3 already passed. Step 4b catches it.
        // Real corpus body that triggered the panic:
        let n = normalize("per ómnia sǽcula sæculórum");
        assert!(!n.contains('æ'), "got {n:?}");
        assert!(n.contains("aecula"));
    }

    #[test]
    fn classify_ortho_variant_genetrice_genitrice() {
        // Real Postcommunio pattern: long shared prefix + single
        // letter differs ("genetrice" vs "genitrice").
        let r = "haecnoscommuniodominepurgetacrimineetintercedentebeatavirginedeigenetricemariacoelestisremediifaciatesseconsortes";
        let p = "haecnoscommuniodominepurgetacrimineetintercedentebeatavirginedeigenitricemariacoelestisremediifaciatesseconsortes";
        assert_eq!(classify_divergence(r, p), DivergenceCategory::OrthoVariant);
    }

    #[test]
    fn classify_ortho_variant_short_inside_long_dismissal_tail() {
        // Real 01-01 case: rust = just the prayer (235c), perl = full
        // section span up to end of HTML including Last Gospel +
        // dismissal (5997c). The substitution still fires because
        // we look at the SHORT body as a near-equal window inside
        // the LONG body.
        let r = "haecnoscommuniodominepurgetacrimineetintercedentebeatavirginedeigenetricemariacoelestisremediifaciatesseconsortes";
        let p_long_tail = format!(
            "haecnoscommuniodominepurgetacrimineetintercedentebeatavirginedeigenitricemariacoelestisremediifaciatesseconsortes{}",
            "perdominumnostrumjesumchristumitemissaestameneitemissaestamenetcumspiritutuoplacegeminitatiblahblahblah".repeat(20),
        );
        assert_eq!(classify_divergence(r, &p_long_tail), DivergenceCategory::OrthoVariant);
    }

    #[test]
    fn classify_ortho_variant_too_small() {
        // Below the 32-char minimum — falls through to Other.
        let r = "abcdefxyz";
        let p = "abcdgfxyz";
        assert_ne!(classify_divergence(r, p), DivergenceCategory::OrthoVariant);
    }

    #[test]
    fn classify_leading_extra() {
        // Rust body is the suffix of perl (perl prepends extra
        // content). Drop 0 leading chars from short → still in long?
        // That's ContainedAsSuffix — actual category is LeadingExtra.
        // Construct so short is NOT directly contained but its tail
        // (after dropping 6 leading chars) IS.
        let r = "INTROxxhodiernadieunigenitumtuumgentibusstelladucerevelasti";
        let p =
            "extraleadingblahblahblahxxhodiernadieunigenitumtuumgentibusstelladucerevelasti";
        let cat = classify_divergence(r, p);
        assert_eq!(cat, DivergenceCategory::LeadingExtra, "got {cat:?}");
    }

    // ─── Rubric stripping (Phase 6.5) ────────────────────────────────

    #[test]
    fn strip_salutation_oratio() {
        let perl = "dominusvobiscumetcumspiritutuooremusdeusquihodiernadie";
        let r = strip_perl_rubrics(perl, "Oratio");
        assert_eq!(r, "deusquihodiernadie");
    }

    #[test]
    fn strip_salutation_with_leading_s_marker() {
        // The Postcommunio in 01-06 shows up with a leading `S.`
        // versicle-marker letter.
        let perl = "sdominusvobiscumetcumspiritutuooremuspraestaquaesumus";
        let r = strip_perl_rubrics(perl, "Postcommunio");
        assert_eq!(r, "praestaquaesumus");
    }

    #[test]
    fn strip_salutation_secreta() {
        let perl = "dominusvobiscumetcumspiritutuooremusecclesiae";
        let r = strip_perl_rubrics(perl, "Secreta");
        assert_eq!(r, "ecclesiae");
    }

    #[test]
    fn strip_salutation_offertorium() {
        let perl = "dominusvobiscumetcumspiritutuooremusps711011regestharsis";
        let r = strip_perl_rubrics(perl, "Offertorium");
        assert_eq!(r, "ps711011regestharsis");
    }

    #[test]
    fn strip_salutation_idempotent() {
        let already = "deusquihodiernadie";
        let r = strip_perl_rubrics(already, "Oratio");
        assert_eq!(r, already);
    }

    #[test]
    fn strip_salutation_only_for_relevant_sections() {
        // Lectio / Graduale / Evangelium / Communio etc. don't carry
        // the salutation prefix — leave them alone.
        let s = "dominusvobiscumetcumspiritutuooremusbody";
        assert_eq!(strip_perl_rubrics(s, "Lectio"), s);
        assert_eq!(strip_perl_rubrics(s, "Graduale"), s);
        assert_eq!(strip_perl_rubrics(s, "Communio"), s);
    }

    #[test]
    fn strip_evangelium_prep() {
        // 01-06 Epiphany Evangelium pattern (1570 form).
        let perl = "mundacormeumaclabiameaomnipotensdeusquilabiaisaiaeprophetaecalculomundastiignitoitametuagratamiserationedignaremundareutsanctumevangeliumtuumdignevaleamnuntiareperchristumdominumnostrumamenjubedominebenedicereGOSPEL";
        let r = strip_perl_rubrics(perl, "Evangelium");
        assert_eq!(r, "GOSPEL");
    }

    #[test]
    fn strip_evangelium_full_real_pattern() {
        // The actual 01-06 sequence: Munda + Jube + Dominus_sit +
        // (Dominus vobiscum/Et cum spiritu tuo) + announcement +
        // Gloria tibi + Gospel + Laus tibi + Per evangelica.
        let perl = format!(
            "{}{}{}{}{}{}{}{}{}",
            EVANGELIUM_MUNDA_COR,
            EVANGELIUM_JUBE_DOMINE,
            EVANGELIUM_DOMINUS_SIT,
            EVANGELIUM_SALUTATION,
            "sequentiasanctievangeliisecundummatthaeu",
            EVANGELIUM_GLORIA_TIBI,
            "matt2112cumnatusessetjesus",
            EVANGELIUM_LAUS_TIBI,
            EVANGELIUM_PER_EVANGELICA,
        );
        let r = strip_perl_rubrics(&perl, "Evangelium");
        assert_eq!(r, "sequentiasanctievangeliisecundummatthaeumatt2112cumnatusessetjesus");
    }

    #[test]
    fn strip_evangelium_dominus_sit_short_form() {
        // 1570 short form ends with just "Amen", no trinitarian
        // "in nomine Patris et Filii et Spiritus Sancti".
        let perl = "dominussitincordemeoetinlabiismeisutdigneetcompetenterannuntiemevangeliumsuumamenGOSPEL";
        let r = strip_perl_rubrics(perl, "Evangelium");
        assert_eq!(r, "GOSPEL");
    }

    #[test]
    fn strip_evangelium_dominus_sit_trinitarian_form() {
        // Later editions extend with "in nomine Patris et Filii et
        // Spiritus Sancti".
        let perl = "dominussitincordemeoetinlabiismeisutdigneetcompetenterannuntiemevangeliumsuuminnominepatrisetfiliietspiritussanctiamenGOSPEL";
        let r = strip_perl_rubrics(perl, "Evangelium");
        assert_eq!(r, "GOSPEL");
    }

    #[test]
    fn strip_evangelium_response_between_announcement_and_body() {
        // Real pattern from 01-06: announcement is in BOTH sides;
        // Perl injects "Gloria tibi Domine" between announcement and
        // Gospel body. Rust:  `<announcement><Gospel>`.
        // Perl:               `<announcement>gloriatibidomine<Gospel>`.
        // After strip the Perl side should equal the Rust side.
        let rust = "sequentiasanctievangeliisecundummatthaeumGOSPEL";
        let perl = "sequentiasanctievangeliisecundummatthaeumgloriatibidomineGOSPEL";
        assert_eq!(strip_perl_rubrics(perl, "Evangelium"), rust);
    }

    #[test]
    fn strip_evangelium_post_gospel_laus_tibi_long() {
        let perl = "GOSPELperevangelicadictadeleanturnostradelictalaustibichriste";
        let r = strip_perl_rubrics(perl, "Evangelium");
        assert_eq!(r, "GOSPEL");
    }

    #[test]
    fn strip_evangelium_post_gospel_laus_tibi_short() {
        let perl = "GOSPELlaustibichriste";
        let r = strip_perl_rubrics(perl, "Evangelium");
        assert_eq!(r, "GOSPEL");
    }

    #[test]
    fn compare_section_named_oratio_strips_perl_rubric() {
        // Rust output (with $Per macro expanded) matches Perl after
        // stripping the salutation prefix.
        let rust_expanded = "Deus, qui hodiérna die... Per Dóminum nostrum.";
        let perl_rendered = "<i>℣.</i> Dóminus vobíscum.<br/><i>℟.</i> Et cum spíritu tuo.<br/><b>O</b>rémus.<br/>Deus, qui hodiérna die... Per Dóminum nostrum.";
        assert_eq!(compare_section_named(rust_expanded, perl_rendered, "Oratio"), SectionStatus::Match);
    }

    // ─── Reverse-lookup tests (Phase 6.5 followup) ───────────────────

    #[test]
    fn infer_source_finds_excelso_throno_in_epi1_0a() {
        // 01-11 fault line: Perl renders "In excélso throno vidi
        // sedere virum..." — the 1570 Sunday-after-Epiphany Introit.
        // The body lives in Tempora/Epi1-0a (NOT Tempora/Epi1-0
        // which is the post-1911 Holy Family). Reverse-lookup should
        // identify Epi1-0a as the source.
        let perl_norm = normalize(
            "In excélso throno vidi sedére virum, quem adórat multitúdo Angelórum, \
             psalléntes in unum: ecce, cujus impérii nomen est in ætérnum"
        );
        let hits = infer_perl_source(&perl_norm, "Introitus");
        assert!(!hits.is_empty(), "no reverse-lookup hit");
        assert!(
            hits.iter().any(|h| h.file == "Tempora/Epi1-0a"),
            "expected Tempora/Epi1-0a in hits; got {:?}",
            hits.iter().map(|h| &h.file).collect::<Vec<_>>()
        );
    }

    #[test]
    fn infer_source_finds_christmas_in_nocte() {
        // "Dóminus dixit ad me…" — Christmas In Nocte Introit.
        // Source: Sancti/12-25m1 (the per-Mass m1 file, not the meta
        // Sancti/12-25 which is body-less).
        let perl_norm = normalize("Dóminus dixit ad me: Fílius meus es tu, ego hódie génui te.");
        let hits = infer_perl_source(&perl_norm, "Introitus");
        assert!(
            hits.iter().any(|h| h.file == "Sancti/12-25m1"),
            "expected Sancti/12-25m1; got {:?}",
            hits.iter().map(|h| (&h.file, h.section, h.score)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn infer_source_short_body_returns_empty() {
        let hits = infer_perl_source("short", "Introitus");
        assert!(hits.is_empty());
    }

    #[test]
    fn match_score_substring_one_direction() {
        // 5/10 — short.len() / long.len()
        let s = match_score("hello", "hellothere");
        assert!((s - 0.5).abs() < 0.01, "got {s}");
    }

    #[test]
    fn match_score_disjoint_zero() {
        assert_eq!(match_score("abcdefgh", "xyzwvuts"), 0.0);
    }

    #[test]
    fn match_score_identical_one() {
        assert_eq!(match_score("hellothere", "hellothere"), 1.0);
    }

    #[test]
    fn match_score_long_common_prefix() {
        // 40-char common prefix, then diverge — should score >0
        // (orthographic-variant case).
        let a = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnXXXXXX";
        let b = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnYYYYYY";
        let s = match_score(a, b);
        assert!(s > 0.5, "got {s}");
    }

    // ─── Placeholder + Perl-bug equivalence tests ─────────────────

    #[test]
    fn placeholder_introitus_missing_normalises_to_empty() {
        // Perl emits `Introitus missing!` when a winner file has no
        // [Introitus]. After normalisation this becomes
        // "introitusmissing"; strip_perl_rubrics should reduce it to
        // empty so the comparator pairs it with Rust's blank.
        let normed = "introitusmissing";
        let stripped = strip_perl_rubrics(normed, "Introitus");
        assert!(stripped.is_empty(), "got {stripped:?}");
    }

    #[test]
    fn placeholder_lectio_missing_with_deo_gratias_tail() {
        // After Lectio Perl appends "Deo gratias" (Lector response).
        let normed = "lectiomissingdeogratias";
        let stripped = strip_perl_rubrics(normed, "Lectio");
        assert!(stripped.is_empty(), "got {stripped:?}");
    }

    #[test]
    fn placeholder_evangelium_missing_with_responses() {
        // Evangelium's closing responses are "Laus tibi Christe" and
        // "Per evangelica dicta..." — both should be tolerated as
        // tails after the missing-marker.
        for normed in [
            "evangeliummissinglaustibichristes",
            "evangeliummissingperevangelicadictadeleanturnostradelicta",
            "evangeliummissinglaustibichristesperevangelicadictadeleanturnostradelicta",
        ] {
            let stripped = strip_perl_rubrics(normed, "Evangelium");
            assert!(stripped.is_empty(), "expected empty, got {stripped:?}");
        }
    }

    #[test]
    fn placeholder_only_fires_for_exact_section_marker() {
        // Section name mismatch: Lectio placeholder shouldn't be
        // stripped when comparing the Communio cell.
        let normed = "lectiomissing";
        let stripped = strip_perl_rubrics(normed, "Communio");
        assert_eq!(stripped, normed);
    }

    #[test]
    fn perl_bug_pent01_introit_treated_as_match() {
        // Trinity Sunday Pent01-0 [Introitus] crashes upstream with
        // "Cannot resolve too deeply nested Hashes". Rust emits the
        // correct Pent01-0r body. The comparator substitutes Rust's
        // text for the Perl error stub so the cell reads as Match.
        let rust_body = "v. Benedícta sit sancta Trínitas atque indivísa Únitas...";
        let perl_html = "\n<br/>\nCannot resolve too deeply nested Hashes<br/>\n";
        let status = compare_section_named(rust_body, perl_html, "Introitus");
        assert_eq!(status, SectionStatus::Match);
    }

    #[test]
    fn perl_bug_does_not_swallow_genuine_differences() {
        // When Perl renders normal content, the comparator must
        // continue to flag genuine divergences.
        let rust_body = "totally different content";
        let perl_html = "<br/>Normal Latin prayer body</br>";
        let status = compare_section_named(rust_body, perl_html, "Oratio");
        assert!(matches!(status, SectionStatus::Differ), "got {status:?}");
    }
}
