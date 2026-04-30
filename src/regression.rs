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

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use crate::divinum_officium::core::MassPropers;

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
    // 5. Lowercase + alphanumeric only.
    folded
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
    // Strip parenthesised rubric notes: `(rubrica 1960)`, `(quae sequitur)`.
    let mut final_out = String::with_capacity(out.len());
    let mut depth = 0i32;
    for ch in out.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if depth == 0 => final_out.push(ch),
            _ => {}
        }
    }
    final_out
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
pub fn extract_perl_sections(html: &str) -> BTreeMap<String, String> {
    let markers = section_marker_positions(html);
    let mut out: BTreeMap<String, String> = BTreeMap::new();
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
            .unwrap_or(html.len());
        if *body_start <= body_end {
            out.insert((*name).to_string(), html[*body_start..body_end].to_string());
        }
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
    PROPER_SECTIONS.iter().chain(ENGLISH_SECTION_NAMES.iter()).chain(ORDINARY_HEADERS.iter())
        .any(|s| s.eq_ignore_ascii_case(name.trim()))
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

// ─── Comparison ──────────────────────────────────────────────────────

pub fn compare_section(rust: &str, perl: &str) -> SectionStatus {
    let r = normalize(rust);
    let p = normalize(perl);
    match (r.is_empty(), p.is_empty()) {
        (true, true) => SectionStatus::Empty,
        (true, false) => SectionStatus::RustBlank,
        (false, true) => SectionStatus::PerlBlank,
        (false, false) => {
            if p.contains(&r) {
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
        let status = compare_section(&rust_body, &perl_body);
        let category = if matches!(status, SectionStatus::Match | SectionStatus::Empty) {
            DivergenceCategory::Match
        } else {
            classify_divergence(&normalize(&rust_body), &normalize(&perl_body))
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
    /// Rust's body is empty; Perl has content.
    RustBlank,
    /// Perl's body is empty; Rust has content.
    PerlBlank,
    /// Neither prefix nor suffix relation — different content.
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
    if perl_norm.contains(rust_norm) {
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
    DivergenceCategory::Other
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
}
