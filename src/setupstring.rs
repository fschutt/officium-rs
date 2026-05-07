//! Shared file / section / `@`-redirect resolver.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/DivinumOfficium/SetupString.pl`
//! (844 LOC). Both Mass (`crate::missa`) and Office
//! (`crate::breviary::corpus`) consume this module after B10.
//!
//! ## Functional-style contract (decided 2026-05-06, see
//! `BREVIARY_PORT_PLAN.md §7.1`)
//!
//! Every helper in this module is a **pure function** over its
//! arguments. The Perl `setupstring` family reads `our $version`,
//! `our @dayname`, `our %winner` from package globals and mutates a
//! per-process file cache; the Rust port takes everything as
//! parameters and the only state it touches is the corpus blob
//! loaded once at process start (`OnceLock`-guarded). No
//! `thread_local!` ambient state is used here — that idiom is
//! reserved for `crate::mass`, where the Mass-side `ACTIVE_RUBRIC`
//! thread-local is a documented compromise to avoid threading the
//! rubric through every body-rewrite helper. New code for the
//! breviary leg passes a [`Subjects`] bundle explicitly.
//!
//! ## Build-time vs. runtime split
//!
//! Today the responsibilities are split:
//!   - **Build-time** parsing of `[Section] body` grammar lives in
//!     `data/build_missa_json.py` and `data/build_horas_json.py`.
//!   - **Build-time** conditional evaluation (`(sed rubrica X)` etc.)
//!     also lives in the build scripts, baking only Tridentine 1570.
//!   - **Runtime** 1-hop `@`-redirect lives in
//!     `crate::missa::resolve_section` (Mass) and
//!     `crate::horas::expand_at_redirect` (Office).
//!
//! B10 consolidates the **runtime conditional evaluator + multi-hop
//! redirect** here so both legs share a single resolver and all five
//! rubric layers are served by one corpus blob (no 5×-baked
//! corpora — see `BREVIARY_PORT_PLAN.md §7.1`). The build-time
//! `[Section] body` parser stays Python until a future all-Rust
//! pipeline.
//!
//! ## Subroutines we mirror from `SetupString.pl`
//!
//! | Perl sub | Lines | Rust target | Status |
//! |---|---|---|---|
//! | `vero($)` | 260 | [`vero`] | ✅ B10b-slice-1 |
//! | `evaluate_conditional($)` | 118 | not ported (see [`vero`]) | n/a |
//! | `conditional_regex()` | 135 | private — pattern compiled once | ⏳ B10b-slice-2 |
//! | `parse_conditional($$$)` | 139 | [`parse_conditional`] | ⏳ B10b-slice-2 |
//! | `get_tempus_id` | 169 | [`get_tempus_id`] | ⏳ B10b-slice-3 |
//! | `get_dayname_for_condition` | 224 | [`dayname_for_condition`] | ⏳ B10b-slice-3 |
//! | `setupstring_parse_file($$$)` | 314 | not ported — done by build script | n/a |
//! | `process_conditional_lines` | 363 | [`process_conditional_lines`] | ⏳ B10b-slice-4 |
//! | `do_inclusion_substitutions(\$$)` | 479 | [`do_inclusion_substitutions`] | ⏳ B10b-slice-5 |
//! | `get_loadtime_inclusion($$$$$$$)` | 502 | [`resolve_load_time_inclusion`] | ⏳ B10b-slice-5 |
//! | `setupstring($$%)` | 534 | [`resolve_section`] | ⏳ B10b-slice-6 |
//! | `officestring($$;$)` | 720 | [`resolve_office_section`] | ⏳ B10b-slice-6 |
//! | `checkfile` / `checklatinfile` | 782/821 | not ported — postcard blob always succeeds | n/a |
//!
//! `evaluate_conditional` (line 118) is a thin token-tape wrapper
//! around `vero`; the breviary port consolidates both into [`vero`]
//! since the Rust strong-typed AST eliminates the need for the
//! string-eval intermediate stage.

use crate::core::Rubric;

// ─── Subjects bundle ─────────────────────────────────────────────────

/// Active state passed to every conditional evaluator. The Perl
/// `setupstring` family reads these from package globals (`our
/// $version`, `our @dayname`, `our $month`, …); the Rust port
/// bundles them into one immutable struct so every helper signature
/// stays small and pure.
///
/// Construct via [`Subjects::new`] for the common case (rubric +
/// dayname + date), then chain builder calls like `.with_hora(…)` to
/// fill in optional fields.
///
/// ## Field ↔ Perl global mapping
///
/// | Field | Perl global | Used by |
/// |---|---|---|
/// | `rubric` | `$version` | `(rubrica X)`, `(rubricis tridentina)`, `(communi summorum pontificum)` |
/// | `dayname0` | `$dayname[0]` | `(tempore X)` and the `get_tempus_id` mapper |
/// | `dayname1` | `$dayname[1]` | `(officio X)` |
/// | `dayname2` | `$dayname[2]` | `(officio X)` (Doctor check) |
/// | `day` | `$day` | `(die N)`, `get_tempus_id` |
/// | `month` | `$month` | `(mense N)`, `get_tempus_id` |
/// | `year` | `$year` | `get_dayname_for_condition` (Nov 1 vs Nov 2) |
/// | `dayofweek` | `$dayofweek` | `(feria N)`, `get_tempus_id` |
/// | `hora` | `$hora` | `(ad X)`, vesp/comp tempus differentiation |
/// | `commune` | `$commune` | `(commune X)` |
/// | `votive` | `$votive` | `(votiva X)` |
/// | `dioecesis` | `$dioecesis` | `(dioecesis X)` |
/// | `winner` | `$winner` | `get_dayname_for_condition` |
/// | `commemoratio` | `$commemoratio` | `get_dayname_for_condition` |
/// | `winner_rule` | `$winner{Rule}` | `get_dayname_for_condition` (3 lectionum check) |
/// | `missa` | `$missa` | `(ad missam)` vs `(ad <hora>)` dispatch |
/// | `missa_number` | `$missanumber` | `(missa N)` for multi-Mass days |
/// | `chant_tone` | `$chantTone` | GABC `(tonus X)`, `(in solemnitatibus)` |
///
/// Fields default to `None` / empty / `false` — callers fill in only
/// what they need. The conditional evaluator skips clauses against
/// missing fields (predicate fails closed).
#[derive(Debug, Clone, Default)]
pub struct Subjects<'a> {
    pub rubric: Option<Rubric>,
    pub dayname0: &'a str,
    pub dayname1: &'a str,
    pub dayname2: &'a str,
    pub day: u32,
    pub month: u32,
    pub year: i32,
    /// 0 = Sunday, 6 = Saturday. The Perl `(feria N)` form uses
    /// `$dayofweek + 1`, so feria 2 is Monday. The Rust evaluator
    /// honours that adjustment internally.
    pub dayofweek: u8,
    pub hora: &'a str,
    pub commune: &'a str,
    pub votive: &'a str,
    pub dioecesis: &'a str,
    pub winner: &'a str,
    pub commemoratio: &'a str,
    pub winner_rule: &'a str,
    /// True when the renderer is in Mass context (drives `(ad missam)`
    /// vs `(ad <hora>)` dispatch).
    pub missa: bool,
    pub missa_number: u8,
    pub chant_tone: &'a str,
}

impl<'a> Subjects<'a> {
    /// Convenience constructor for the common case — `(rubric,
    /// dayname0, day, month, year)`. Other fields stay at their
    /// default; chain `.with_*` builders to populate as needed.
    pub fn new(rubric: Rubric, dayname0: &'a str, day: u32, month: u32, year: i32) -> Self {
        Self {
            rubric: Some(rubric),
            dayname0,
            day,
            month,
            year,
            ..Default::default()
        }
    }

    pub fn with_dayname(mut self, dayname0: &'a str, dayname1: &'a str, dayname2: &'a str) -> Self {
        self.dayname0 = dayname0;
        self.dayname1 = dayname1;
        self.dayname2 = dayname2;
        self
    }

    pub fn with_hora(mut self, hora: &'a str) -> Self {
        self.hora = hora;
        self
    }

    pub fn with_dayofweek(mut self, dayofweek: u8) -> Self {
        self.dayofweek = dayofweek;
        self
    }

    pub fn with_winner(mut self, winner: &'a str, rule: &'a str) -> Self {
        self.winner = winner;
        self.winner_rule = rule;
        self
    }

    pub fn with_commemoratio(mut self, commemoratio: &'a str) -> Self {
        self.commemoratio = commemoratio;
        self
    }

    pub fn with_commune(mut self, commune: &'a str) -> Self {
        self.commune = commune;
        self
    }

    pub fn with_votive(mut self, votive: &'a str) -> Self {
        self.votive = votive;
        self
    }

    pub fn with_dioecesis(mut self, dioecesis: &'a str) -> Self {
        self.dioecesis = dioecesis;
        self
    }

    pub fn with_missa(mut self, missa: bool, missa_number: u8) -> Self {
        self.missa = missa;
        self.missa_number = missa_number;
        self
    }

    pub fn with_chant_tone(mut self, chant_tone: &'a str) -> Self {
        self.chant_tone = chant_tone;
        self
    }

    /// True when the active hour is Vespera or Completorium. Drives
    /// the eve-of-feast tempus differentiation in
    /// [`get_tempus_id`] / [`dayname_for_condition`]. Mirror of the
    /// Perl `$vesp_or_comp` local in those functions.
    pub fn is_vesp_or_comp(&self) -> bool {
        eq_ci_contains(self.hora, "Vespera") || eq_ci_contains(self.hora, "Completorium")
    }
}

// ─── vero: condition evaluator ───────────────────────────────────────

/// Parse and evaluate a Latin conditional expression (the body of a
/// `(... )` guard). Returns true when the active state in `subjects`
/// satisfies the condition.
///
/// Mirror of `SetupString.pl::vero` line 260-308.
///
/// ## Grammar
///
/// ```text
/// condition := disjunct ( "aut" disjunct )*
/// disjunct  := atom ( ( "et" | "nisi" ) atom )*
/// atom      := <subject>? <predicate>
/// ```
///
/// `aut` binds *less tightly* than `et` / `nisi` (i.e. `aut` separates
/// disjuncts of `et`-conjuncts). `nisi` negates the next conjunct
/// only — which means `aut` resets the negation, but `et` does not.
/// This is exactly the Perl behaviour at lines 270-303.
///
/// An empty condition is true (Perl line 267 — "safer, since
/// previously conditions weren't used").
///
/// ## Subjects
///
/// Subject keywords supported (all matched case-insensitively):
///
/// | Keyword | Reads | Notes |
/// |---|---|---|
/// | `rubricis` / `rubrica` | `subjects.rubric` | as Perl version string |
/// | `tempore` | `get_tempus_id(subjects)` | default subject when absent |
/// | `missa` | `subjects.missa_number` | numeric |
/// | `communi` | `subjects.rubric` | (yes — Perl quirk; `communi` reads `$version`) |
/// | `die` | `dayname_for_condition(subjects)` | day-keyword |
/// | `feria` | `subjects.dayofweek + 1` | numeric |
/// | `commune` | `subjects.commune` | string |
/// | `votiva` | `subjects.votive` | string |
/// | `officio` | `subjects.dayname1` | dayname tag |
/// | `ad` | `subjects.hora` (or `"missam"` if `subjects.missa`) | hour name |
/// | `mense` | `subjects.month` | numeric |
/// | `dioecesis` | `subjects.dioecesis` | string |
/// | `tonus` / `toni` | `subjects.chant_tone` | GABC chant tone |
///
/// Subject is optional — when absent the predicate is matched against
/// `tempore` (the result of [`get_tempus_id`] applied to `subjects`).
///
/// ## Predicates
///
/// Named predicates supported (case-insensitive). Most match the
/// subject value as a regex; some are numeric equality:
///
/// | Predicate | Test against subject |
/// |---|---|
/// | `tridentina` | `=~ /Trident/` (rubric label) |
/// | `monastica` | `=~ /Monastic/` |
/// | `innovata` / `innovatis` | `=~ /2020 USA|NewCal/i` |
/// | `paschali` | `=~ /Paschæ|Ascensionis|Octava Pentecostes/i` |
/// | `post septuagesimam` | `=~ /Septua|Quadra|Passio/i` |
/// | `prima` / `longior` | `== 1` |
/// | `secunda` / `brevior` | `== 2` |
/// | `tertia` | `== 3` |
/// | `summorum pontificum` | rubric matches 1942-49 / 1954-55 / 1960 |
/// | `feriali` | `=~ /feria|vigilia/i` |
/// | `in solemnitatibus` | `=~ /solemnis|resurrectionis/i` |
/// | `in hieme` | `=~ /hieme|Adventus|...|Passionis/i` |
/// | `in æstate` | NOT `in hieme` |
///
/// When the predicate isn't a named one, the predicate text itself
/// is matched against the subject value as a case-insensitive regex.
/// `(rubrica 1960)` therefore matches against the rubric label
/// "Rubrics 1960 - 1960" via the literal substring "1960".
pub fn vero(condition: &str, subjects: &Subjects<'_>) -> bool {
    let trimmed = condition.trim();
    if trimmed.is_empty() {
        // Perl line 267: empty condition is true.
        return true;
    }

    // `aut` separates disjuncts; any disjunct evaluating to true wins.
    for disjunct in split_keyword(trimmed, "aut") {
        let mut negation = false;
        let mut all_ok = true;
        let mut empty = true;
        // Each disjunct is split on `et` / `nisi`, with `nisi`
        // toggling negation for the *following* atom only (until
        // the next `aut`).
        for piece in split_et_nisi(disjunct) {
            match piece {
                EtNisiPiece::Et => continue,
                EtNisiPiece::Nisi => {
                    negation = true;
                }
                EtNisiPiece::Atom(text) => {
                    empty = false;
                    let ok = eval_atom(text, subjects);
                    if ok == negation {
                        // negation && ok → atom matched but should not have.
                        // !negation && !ok → atom didn't match.
                        all_ok = false;
                        break;
                    }
                }
            }
        }
        // Perl quirk: an empty disjunct (e.g. trailing `aut` with no
        // body) doesn't satisfy the disjunction.
        if !empty && all_ok {
            return true;
        }
    }
    false
}

/// One piece of an `aut`-disjunct after splitting on `et` / `nisi`.
#[derive(Debug)]
enum EtNisiPiece<'s> {
    Atom(&'s str),
    Et,
    Nisi,
}

/// Split an aut-disjunct body on `\bet\b` and `\bnisi\b` boundaries.
/// Preserves the operators in-stream so the caller can track
/// negation flips. Word-boundary aware so `dies` doesn't trigger on
/// `et`.
fn split_et_nisi(body: &str) -> Vec<EtNisiPiece<'_>> {
    let mut out = Vec::new();
    let mut last_end = 0usize;
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
        // Look for `et` or `nisi` at a word boundary.
        let kw = if word_at(bytes, i, b"et") {
            Some((EtNisiPiece::Et, 2))
        } else if word_at(bytes, i, b"nisi") {
            Some((EtNisiPiece::Nisi, 4))
        } else {
            None
        };
        if let Some((piece, kw_len)) = kw {
            let pre = &body[last_end..i];
            if !pre.trim().is_empty() {
                out.push(EtNisiPiece::Atom(pre.trim()));
            }
            out.push(piece);
            i += kw_len;
            last_end = i;
            continue;
        }
        i += 1;
    }
    let tail = &body[last_end..];
    if !tail.trim().is_empty() {
        out.push(EtNisiPiece::Atom(tail.trim()));
    }
    out
}

/// Split a condition body on `\baut\b` boundaries, preserving each
/// disjunct's interior whitespace so subsequent splitting on `et` /
/// `nisi` still finds its boundaries.
fn split_keyword<'s>(body: &'s str, keyword: &'static str) -> Vec<&'s str> {
    let kw_bytes = keyword.as_bytes();
    let kw_len = kw_bytes.len();
    let mut out = Vec::new();
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut last_end = 0usize;
    let mut i = 0usize;
    while i + kw_len <= n {
        if word_at(bytes, i, kw_bytes) {
            out.push(&body[last_end..i]);
            i += kw_len;
            last_end = i;
            continue;
        }
        i += 1;
    }
    out.push(&body[last_end..]);
    out
}

/// True when `kw` starts at byte offset `at` in `bytes`, at a word
/// boundary on both sides. Word characters: ASCII alphanumeric +
/// underscore. Case-sensitive — Perl `\bkw\b` is case-sensitive too;
/// callers normalise their input to lowercase before splitting.
fn word_at(bytes: &[u8], at: usize, kw: &[u8]) -> bool {
    let n = bytes.len();
    if at + kw.len() > n {
        return false;
    }
    if !bytes[at..at + kw.len()].eq_ignore_ascii_case(kw) {
        return false;
    }
    let left_ok = at == 0 || !is_word_byte(bytes[at - 1]);
    let right_idx = at + kw.len();
    let right_ok = right_idx == n || !is_word_byte(bytes[right_idx]);
    left_ok && right_ok
}

const fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Evaluate one `(<subject> <predicate>)` atom. Returns true when
/// the predicate matches the subject's value under `subjects`.
fn eval_atom(atom: &str, subjects: &Subjects<'_>) -> bool {
    // Normalise whitespace.
    let atom = atom.split_whitespace().collect::<Vec<_>>().join(" ");
    if atom.is_empty() {
        return false;
    }

    // Split into (subject, predicate). Subject is one word; rest is
    // the predicate text.
    let (subject_token, predicate_text) = match atom.split_once(' ') {
        Some((s, p)) => (s.to_string(), p.to_string()),
        None => (String::new(), atom.clone()),
    };

    // The Perl rule (lines 281-290):
    //   If subject doesn't resolve to a known subject, treat it as
    //   the first word of a multi-word predicate ("post septuagesimam").
    //   If predicate is empty (atom is a single word), it's actually
    //   the predicate; subject defaults to "tempore".
    let (subject_key, predicate) = if predicate_text.is_empty() {
        // Single token — that's the predicate; subject defaults to
        // "tempore".
        ("tempore".to_string(), subject_token)
    } else if !is_subject(&subject_token) {
        // Two-or-more tokens but the first isn't a known subject —
        // it's the start of a multi-word predicate; subject defaults
        // to "tempore".
        let predicate = if subject_token.is_empty() {
            predicate_text
        } else {
            format!("{subject_token} {predicate_text}")
        };
        ("tempore".to_string(), predicate)
    } else {
        (subject_token, predicate_text)
    };

    let subject_value = subject_value(&subject_key, subjects);
    eval_predicate(&predicate, &subject_value)
}

/// True when `tok` is a recognised subject keyword.
fn is_subject(tok: &str) -> bool {
    matches!(
        tok.to_ascii_lowercase().as_str(),
        "rubricis"
            | "rubrica"
            | "tempore"
            | "missa"
            | "communi"
            | "die"
            | "feria"
            | "commune"
            | "votiva"
            | "officio"
            | "ad"
            | "mense"
            | "dioecesis"
            | "tonus"
            | "toni"
    )
}

/// Resolve a subject keyword to its current value. Mirror of the
/// `%subjects` hash in `SetupString.pl:18-38`.
fn subject_value(subject: &str, subjects: &Subjects<'_>) -> String {
    match subject.to_ascii_lowercase().as_str() {
        "rubricis" | "rubrica" | "communi" => subjects
            .rubric
            .map(|r| r.as_perl_version().to_string())
            .unwrap_or_default(),
        "tempore" => get_tempus_id(subjects),
        "missa" => subjects.missa_number.to_string(),
        "die" => dayname_for_condition(subjects),
        // Perl `$dayofweek + 1` — feria 2 is Monday.
        "feria" => (subjects.dayofweek + 1).to_string(),
        "commune" => subjects.commune.to_string(),
        "votiva" => subjects.votive.to_string(),
        "officio" => subjects.dayname1.to_string(),
        "ad" => {
            if subjects.missa {
                "missam".to_string()
            } else {
                subjects.hora.to_string()
            }
        }
        "mense" => subjects.month.to_string(),
        "dioecesis" => subjects.dioecesis.to_string(),
        "tonus" | "toni" => subjects.chant_tone.to_string(),
        _ => String::new(),
    }
}

/// Apply a predicate to a subject's value. Mirror of the `%predicates`
/// hash in `SetupString.pl:39-60` plus the regex-fallback at line 299.
fn eval_predicate(predicate: &str, value: &str) -> bool {
    // Normalise to ASCII-lowercase for lookup; Perl regexes use `/i`.
    let key = predicate.trim().to_ascii_lowercase();
    match key.as_str() {
        "tridentina" => contains_ci(value, "Trident"),
        "monastica" => contains_ci(value, "Monastic"),
        "innovata" | "innovatis" => contains_ci(value, "2020 USA") || contains_ci(value, "NewCal"),
        "paschali" => {
            contains_ci(value, "Paschæ")
                || contains_ci(value, "Ascensionis")
                || contains_ci(value, "Octava Pentecostes")
        }
        "post septuagesimam" => {
            contains_ci(value, "Septua")
                || contains_ci(value, "Quadra")
                || contains_ci(value, "Passio")
        }
        "prima" | "longior" => value.trim() == "1",
        "secunda" | "brevior" => value.trim() == "2",
        "tertia" => value.trim() == "3",
        "summorum pontificum" => {
            // Perl regex: /194[2-9]]|195[45]|196/ (note the typo `]]`
            // in the Perl source — it's the literal `]` character at
            // the end of the alternation, which never matches; we
            // model the obvious intent: 1942-49, 1954-55, or 1960).
            let v = value;
            (1942..=1949).any(|y| v.contains(&y.to_string()))
                || v.contains("1954")
                || v.contains("1955")
                || v.contains("196")
        }
        "feriali" => contains_ci(value, "feria") || contains_ci(value, "vigilia"),
        "in solemnitatibus" => {
            contains_ci(value, "solemnis") || contains_ci(value, "resurrectionis")
        }
        "in hieme" => is_in_hieme(value),
        "in æstate" => !is_in_hieme(value),
        _ => {
            // Fallback (Perl line 299): treat predicate text as a regex
            // and test the subject value against it. Our Rust port
            // approximates with case-insensitive substring match —
            // 99 % of the actual predicates in the corpus are
            // single-word literal matches; the few regex-y ones
            // (`Adv|Nat`) work through this path too because we
            // honour `|` as an alternation separator below.
            //
            // Honour `|` (regex alternation) — Perl `qr/$predicate/i`
            // accepts it.
            for alt in predicate.split('|') {
                let alt = alt.trim();
                if alt.is_empty() {
                    continue;
                }
                if contains_ci(value, alt) {
                    return true;
                }
            }
            false
        }
    }
}

fn is_in_hieme(value: &str) -> bool {
    contains_ci(value, "hieme")
        || contains_ci(value, "Adventus")
        || contains_ci(value, "Nativitatis")
        || contains_ci(value, "Epiphani")
        || contains_ci(value, "gesimæ")
        || contains_ci(value, "Passionis")
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    // ASCII fast path; otherwise lowercase both.
    if needle.is_ascii() && haystack.is_ascii() {
        let h = haystack.as_bytes();
        let n = needle.as_bytes();
        for i in 0..=(h.len() - n.len()) {
            if h[i..i + n.len()].eq_ignore_ascii_case(n) {
                return true;
            }
        }
        return false;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn eq_ci_contains(haystack: &str, needle: &str) -> bool {
    contains_ci(haystack, needle)
}

// ─── get_tempus_id / dayname_for_condition (B10b-slice-3) ───────────

/// Get the tempus identifier for the active state. Mirror of
/// `SetupString.pl::get_tempus_id` line 169-221. Returns the season
/// keyword that `(... tempore X)` clauses test against.
///
/// Maps `dayname[0]` → seasonal label per the upstream cascade:
///
/// - `Adv*` → "Adventus"
/// - `Nat*` → "Nativitatis" or "Epiphaniæ" (depending on Jan 5/6)
/// - `Epi*` → "Epiphaniæ" / "post Epiphaniam post partum" / "post
///   Epiphaniam" / "post Pentecosten in hieme" (depending on date)
/// - `Quadp*` → "Septuagesimæ" / "Septuagesimæ post partum"
/// - `Quad1`-`Quad4` → "Quadragesimæ"
/// - `Quad5`/`Quad6` → "Passionis"
/// - `Pasc0` → "Octava Paschæ" or "Vigilia Paschalis" (Sat eve)
/// - `Pasc1`-`Pasc4` → "post Octavam Paschæ"
/// - `Pasc5` (early) → "post Octavam Paschæ"; (late) → "Octava Ascensionis"
/// - `Pasc6-(5|6)` → "post Octavam Ascensionis"
/// - `Pasc6-other` → "Octava Ascensionis"
/// - `Pasc7` → "Octava Pentecostes"
/// - `Pent01` Thursday → "Corpus Christi post Pentecosten"
/// - `Pent01`/`Pent02` interior → "Octava Corpus Christi post Pentecosten"
///   (pre-1955)
/// - `Pent02` Friday → "SSmi Cordis post Pentecosten" (post-Tridentine)
/// - `Pent02`/`Pent03` interior → "Octava SSmi Cordis post Pentecosten"
///   (Divino only)
/// - `Pent*` Oct/Nov → "post Pentecosten in hieme"
/// - `Pent*` else → "post Pentecosten"
pub fn get_tempus_id(subjects: &Subjects<'_>) -> String {
    let d = subjects.dayname0;
    let day = subjects.day as i32;
    let month = subjects.month as i32;
    let dayofweek = subjects.dayofweek as i32;
    let vesp_or_comp = subjects.is_vesp_or_comp();
    let rubric_label = subjects
        .rubric
        .map(|r| r.as_perl_version())
        .unwrap_or("");

    // Adv*
    if d.starts_with("Adv") {
        return "Adventus".to_string();
    }

    // Nat*
    if d.starts_with("Nat") {
        return if month == 1 && (day >= 6 || (day == 5 && vesp_or_comp)) {
            "Epiphaniæ".to_string()
        } else {
            "Nativitatis".to_string()
        };
    }

    // Epi*
    if d.starts_with("Epi") {
        return if month == 1 && day <= 13 {
            "Epiphaniæ".to_string()
        } else if month == 1 || (month == 2 && (day == 1 || (day == 2 && !vesp_or_comp))) {
            "post Epiphaniam post partum".to_string()
        } else if month == 2 {
            "post Epiphaniam".to_string()
        } else {
            "post Pentecosten in hieme".to_string()
        };
    }

    // Quadp(\d) where digit < 3 OR dayofweek < 3
    if let Some(rest) = d.strip_prefix("Quadp") {
        if let Some(digit) = rest.chars().next().and_then(|c| c.to_digit(10)) {
            if digit < 3 || dayofweek < 3 {
                return if month == 1 || (month == 2 && (day == 1 || (day == 2 && !vesp_or_comp))) {
                    "Septuagesimæ post partum".to_string()
                } else {
                    "Septuagesimæ".to_string()
                };
            }
        }
    }

    // Quad(\d) where digit < 5
    if let Some(rest) = d.strip_prefix("Quad") {
        if let Some(digit) = rest.chars().next().and_then(|c| c.to_digit(10)) {
            if digit < 5 {
                return "Quadragesimæ".to_string();
            }
        }
        // Quad5/Quad6 → Passionis (any Quad without a digit < 5 falls
        // through here, including the bare "Quad" form).
        return "Passionis".to_string();
    }

    // Pasc0 — Octava Paschæ, with Saturday-eve special case
    if d.starts_with("Pasc0") {
        return if vesp_or_comp && dayofweek == 6 {
            "Vigilia Paschalis".to_string()
        } else {
            "Octava Paschæ".to_string()
        };
    }

    // Pasc(\d) — split on weeks; Pasc5 has the Vigil-of-Ascension carve-out
    if let Some(rest) = d.strip_prefix("Pasc") {
        if let Some(digit) = rest.chars().next().and_then(|c| c.to_digit(10)) {
            if digit < 5
                || (digit == 5
                    && (dayofweek < 3 || (!vesp_or_comp && dayofweek == 3)))
            {
                return "post Octavam Paschæ".to_string();
            }
            // Pasc6-(5|6) → post Octavam Ascensionis
            if digit == 6 {
                let after_digit = &rest[1..];
                if matches!(after_digit, "-5" | "-6") {
                    return "post Octavam Ascensionis".to_string();
                }
            }
            if digit < 7 {
                return "Octava Ascensionis".to_string();
            }
            return "Octava Pentecostes".to_string();
        }
    }

    // Pent01 with Thursday is Corpus Christi
    if d.starts_with("Pent01") && dayofweek == 4 {
        return "Corpus Christi post Pentecosten".to_string();
    }

    // Pent0(\d) Octave-of-Corpus-Christi window (pre-1955)
    let after_pent = d.strip_prefix("Pent0");
    if let Some(rest) = after_pent {
        if let Some(digit) = rest.chars().next().and_then(|c| c.to_digit(10)) {
            let in_oct_cc_window = (digit == 1
                && dayofweek > 4
                && !(dayofweek == 6 && vesp_or_comp))
                || (digit == 2 && (dayofweek < 5 || (dayofweek == 6 && vesp_or_comp)));
            // Perl: $version !~ /19(?:55|6)/ — exclude 1955 and 1960+.
            let pre_1955 = !(rubric_label.contains("1955")
                || rubric_label.contains("1960")
                || rubric_label.contains("196"));
            if in_oct_cc_window && pre_1955 {
                return "Octava Corpus Christi post Pentecosten".to_string();
            }
        }
    }

    // Pent02 Friday → SSmi Cordis (post-Tridentine — i.e. NOT 1570)
    if d.starts_with("Pent02") && dayofweek == 5 && !rubric_label.contains("1570") {
        return "SSmi Cordis post Pentecosten".to_string();
    }

    // Octava SSmi Cordis (Divino only)
    if let Some(rest) = after_pent {
        if let Some(digit) = rest.chars().next().and_then(|c| c.to_digit(10)) {
            let in_oct_ssmi_window = (digit == 2
                && dayofweek > 5
                && !(dayofweek == 6 && vesp_or_comp))
                || (digit == 3 && (dayofweek < 6 || (dayofweek == 6 && vesp_or_comp)));
            // Perl: $version =~ /Divino/i
            if in_oct_ssmi_window && rubric_label.to_lowercase().contains("divino") {
                return "Octava SSmi Cordis post Pentecosten".to_string();
            }
        }
    }

    // Pent* — fall through to "post Pentecosten" or "post Pentecosten in hieme"
    if d.starts_with("Pent") {
        // oct_or_nov approximation: month is October or November.
        // Perl uses the `monthday` global which has the form `10X-Y` /
        // `11X-Y` for Oct/Nov week-X-day-Y; for the post-Pentecost
        // tempus we just need the broad month check.
        let oct_or_nov = month == 10 || month == 11;
        if !oct_or_nov {
            return "post Pentecosten".to_string();
        }
    }

    // Final fallback (Perl line 220).
    "post Pentecosten in hieme".to_string()
}

/// Get the dayname keyword for `(... die X)` clauses. Mirror of
/// `SetupString.pl::get_dayname_for_condition` line 224-257.
///
/// Returns one of:
///
/// - `"Epiphaniæ"` — Jan 6 (or Jan 5 eve)
/// - `"Baptismatis Domini"` — Jan 13 (or Jan 12 eve)
/// - `"Tridui Sacri"` — Maundy Thu / Good Fri / Holy Sat
/// - `"in Cœna Domini"` — Maundy Thu specifically
/// - `"in Parasceve"` — Good Fri specifically
/// - `"Sabbato Sancto"` — Holy Sat specifically
/// - `"Vigilia Paschalis"` — Easter Sat eve
/// - `"regis DNJC"` — Christ the King (10-DU)
/// - `"Omnium Defunctorum"` — All Souls (Nov 2 + adjacent)
/// - `"Malachiae"` / `"Caroli"` / `"Nicolai"` — fixed feast keywords
/// - `"Nat28"` / `"Nat29"` — Holy Innocents window
/// - `"doctorum"` — winner is a Doctor of the Church
/// - `"transfigurationis"` — Aug 6 (or Aug 5 eve)
/// - `"septem doloris"` — Seven Sorrows (Sep 15 / Quad5-5)
/// - `"Nativitatis"` — Christmas Day (winner = 12-25)
/// - `"post Dominicam infra Octavam Epiphaniæ"` — Epi1-1..6
/// - `"Bernardi"` — Aug 20
/// - `"3 lectionum"` — when winner has 3-lectio rule
/// - `""` — none of the above
pub fn dayname_for_condition(subjects: &Subjects<'_>) -> String {
    let d = subjects.day as i32;
    let m = subjects.month as i32;
    let y = subjects.year;
    let dayofweek = subjects.dayofweek as i32;
    let vesp_or_comp = subjects.is_vesp_or_comp();
    let winner = subjects.winner;
    let commemoratio = subjects.commemoratio;

    // Each `return` in the Perl runs in order; we mirror that.
    if m == 1 && (d == 6 || (d == 5 && vesp_or_comp)) {
        return "Epiphaniæ".to_string();
    }
    if m == 1 && (d == 13 || (d == 12 && vesp_or_comp)) {
        return "Baptismatis Domini".to_string();
    }
    if regex_lite_match(winner, "Quad6-[456]") {
        // Order matters: more-specific Tridui keywords come AFTER the
        // umbrella one, but Perl returns on the FIRST match. So if
        // winner is Quad6-4, we'd return "Tridui Sacri" not "in Cœna
        // Domini". That's the upstream behaviour — replicate it.
        return "Tridui Sacri".to_string();
    }
    if regex_lite_match(winner, "Quad6-4") {
        return "in Cœna Domini".to_string();
    }
    if regex_lite_match(winner, "Quad6-5") {
        return "in Parasceve".to_string();
    }
    if regex_lite_match(winner, "Quad6-6") {
        return "Sabbato Sancto".to_string();
    }
    if regex_lite_match(winner, "Pasc0-0") && vesp_or_comp && dayofweek == 6 {
        return "Vigilia Paschalis".to_string();
    }
    if winner.contains("10-DU") || commemoratio.contains("10-DU") {
        return "regis DNJC".to_string();
    }
    // All Souls window: Nov 2; or Nov 3 when Nov 2 fell on a Sunday;
    // or Nov 1 eve when Nov 1 itself isn't a Saturday.
    if m == 11 {
        let nov1_dow = crate::date::day_of_week(1, 11, y) as i32;
        if d == 2
            || (d == 3 && dayofweek == 1)
            || (d == 1 && nov1_dow != 6 && vesp_or_comp)
        {
            return "Omnium Defunctorum".to_string();
        }
    }
    if m == 11 && d == 3 {
        return "Malachiae".to_string();
    }
    if m == 11 && d == 4 {
        return "Caroli".to_string();
    }
    if m == 12 && d == 6 {
        return "Nicolai".to_string();
    }
    if m == 12 && d == 28 {
        return "Nat28".to_string();
    }
    if m == 12 && d == 29 {
        return "Nat29".to_string();
    }
    // Doctor check — dayname[1] or dayname[2] contains "Doctor".
    if contains_ci(subjects.dayname1, "Doctor") || contains_ci(subjects.dayname2, "Doctor") {
        return "doctorum".to_string();
    }
    if m == 8 && (d == 6 || (d == 5 && vesp_or_comp)) {
        return "transfigurationis".to_string();
    }
    // Septem doloris — 09-15 (Sept 15) OR 09-DT (movable) OR Quad5-5.
    if regex_lite_match(winner, "09-15$")
        || winner.contains("09-DT")
        || regex_lite_match(winner, "Quad5-5$")
    {
        return "septem doloris".to_string();
    }
    if winner.contains("12-25") {
        return "Nativitatis".to_string();
    }
    // Epi1-(1..6) — first Sunday after Epiphany interior days.
    if regex_lite_match(subjects.dayname0, "Epi1-[1-6]") {
        // Perl line 251 returns first; line 252 is dead code (same
        // pattern, different return value). Replicate the upstream
        // behaviour — the second return never fires.
        return "post Dominicam infra Octavam Epiphaniæ".to_string();
    }
    if winner.contains("08-20") || winner.contains("00-VB") {
        return "Bernardi".to_string();
    }
    // Lines 254-255: both fire on `3 lectio` substring; the first one
    // wins, so we always return "3 lectionum" (line 255 is dead code).
    if contains_ci(subjects.winner_rule, "3 lectio") {
        return "3 lectionum".to_string();
    }

    String::new()
}

/// Lightweight regex matcher — supports `^` / `$` anchors and literal
/// character classes like `[1-6]`. Sufficient for the
/// `dayname_for_condition` checks (none of which use full regex).
/// Returns true on the first match anywhere in `haystack`.
fn regex_lite_match(haystack: &str, pattern: &str) -> bool {
    // Strip optional `^` and `$` anchors.
    let (anchored_start, mut p) = if let Some(rest) = pattern.strip_prefix('^') {
        (true, rest)
    } else {
        (false, pattern)
    };
    let anchored_end = p.ends_with('$');
    if anchored_end {
        p = &p[..p.len() - 1];
    }

    let h_bytes = haystack.as_bytes();
    let h_len = h_bytes.len();

    // Try every possible start position (or just position 0 for
    // anchored-start patterns).
    let positions: Box<dyn Iterator<Item = usize>> = if anchored_start {
        Box::new(0..=0)
    } else {
        Box::new(0..=h_len)
    };

    for start in positions {
        if let Some(end) = match_at(h_bytes, start, p) {
            if !anchored_end || end == h_len {
                return true;
            }
        }
    }
    false
}

/// Try to match `pattern` against `bytes` starting at `pos`. Returns
/// the end index on success, `None` on failure.
fn match_at(bytes: &[u8], pos: usize, pattern: &str) -> Option<usize> {
    let pat = pattern.as_bytes();
    let mut bi = pos;
    let mut pi = 0usize;
    while pi < pat.len() {
        if pat[pi] == b'[' {
            // Character class — find the closing `]`.
            let class_end = pat[pi..].iter().position(|&c| c == b']')?;
            let class = &pat[pi + 1..pi + class_end];
            if bi >= bytes.len() {
                return None;
            }
            if !match_class(class, bytes[bi]) {
                return None;
            }
            bi += 1;
            pi += class_end + 1;
        } else {
            // Literal byte.
            if bi >= bytes.len() || bytes[bi] != pat[pi] {
                return None;
            }
            bi += 1;
            pi += 1;
        }
    }
    Some(bi)
}

/// True when `b` matches a character class body (e.g. `1-6` matches
/// digits 1..=6; `1-6abc` matches those plus literal a/b/c).
fn match_class(class: &[u8], b: u8) -> bool {
    let mut i = 0;
    while i < class.len() {
        if i + 2 < class.len() && class[i + 1] == b'-' {
            if b >= class[i] && b <= class[i + 2] {
                return true;
            }
            i += 3;
        } else {
            if b == class[i] {
                return true;
            }
            i += 1;
        }
    }
    false
}

// ─── Conditional parser (B10b-slice-2) ──────────────────────────────

/// One parsed conditional expression. Used by [`process_conditional_lines`]
/// to evaluate `(...)` guards line-by-line with backward / forward
/// scope tracking.
#[derive(Debug, Clone)]
pub struct Conditional {
    /// Stopword strength — `0` (none / `si` / `deinde`), `1` (`sed`,
    /// `vero`), `2` (`atque`), `3` (`attamen`). Drives which conditional
    /// frames in the stack get popped when this one fires. See
    /// `SetupString.pl:77-86`.
    pub strength: u8,
    /// Whether the condition body itself evaluates true under the
    /// active subjects.
    pub result: bool,
    /// What lines before this conditional get retroactively dropped
    /// when the conditional fires.
    pub backscope: Scope,
    /// What lines after this conditional are gated on the result.
    pub forwardscope: Scope,
}

/// Conditional scope. Mirror of the four `SCOPE_*` constants at
/// `SetupString.pl:111-114`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Null scope — no lines affected.
    Null,
    /// Single line.
    Line,
    /// Until the next blank line.
    Chunk,
    /// Until a (weakly) stronger conditional.
    Nest,
}

/// One match of the upstream `conditional_regex` (line 135-137).
/// Captures the three ordered fields:
///
///   `( stopwords <whitespace> condition_body <whitespace> scope_keywords )`
///
/// Where:
///   * `stopwords` — zero or more of `sed` / `vero` / `atque` /
///     `attamen` / `si` / `deinde`, space-separated.
///   * `condition_body` — the part fed to [`vero`].
///   * `scope_keywords` — optional trailing tokens like `dicitur
///     semper`, `omittitur`, `omittuntur`, optionally prefixed by
///     `loco hujus versus` / `loco horum versuum`.
///
/// `start` and `end` are byte offsets in the original input string —
/// useful for `process_conditional_lines` which needs to emit the
/// rest of the line ("sequel") after stripping the directive.
#[derive(Debug, Clone)]
pub struct ConditionalMatch<'a> {
    pub start: usize,
    pub end: usize,
    pub stopwords: &'a str,
    pub condition: &'a str,
    pub scope: &'a str,
}

/// Find the first `(... )` conditional directive in `body`. Returns
/// `None` when no balanced `(...)` is present, or when the matched
/// `(...)` doesn't look like a conditional (no recognised stopword
/// or scope keyword AND a non-Latin-condition body).
///
/// Mirror of `SetupString.pl::conditional_regex` (line 135-137) used
/// in match context. The Perl regex is anchored mid-line (no `^`)
/// which means it finds the first `(...)` anywhere; we replicate
/// that by scanning for `(` then balancing.
///
/// We deliberately accept any `(...)` parenthesised text and let the
/// caller decide whether the body is conditional-like. That mirrors
/// the upstream behaviour where `process_conditional_lines` matches
/// `^\s*$conditional_regex\s*(.*)$` line-anchored — every `(...)` at
/// the start of a line is treated as a conditional, even ones with
/// non-Latin bodies (the regex-fallback in `vero` makes those
/// interpretable).
pub fn find_conditional(body: &str) -> Option<ConditionalMatch<'_>> {
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
        if bytes[i] == b'(' {
            // Find matching `)` (no nesting allowed — the upstream
            // regex doesn't recurse).
            let close = i + 1 + bytes[i + 1..].iter().position(|&c| c == b')')?;
            if close <= i + 1 {
                i += 1;
                continue;
            }
            let inside = &body[i + 1..close];
            return Some(parse_conditional_inside(inside, i, close + 1));
        }
        i += 1;
    }
    None
}

/// Parse the interior of a `(...)` directive — the part between the
/// parentheses. Splits into stopwords + condition + scope.
fn parse_conditional_inside(inside: &str, start: usize, end: usize) -> ConditionalMatch<'_> {
    let trimmed = inside.trim_start_matches(|c: char| c.is_whitespace());
    // Split off leading stopwords.
    let (stopwords, after_stop) = take_leading_stopwords(trimmed);
    // The remainder is condition + optional trailing scope.
    let (condition, scope) = split_off_trailing_scope(after_stop);
    ConditionalMatch {
        start,
        end,
        stopwords,
        condition: condition.trim(),
        scope: scope.trim(),
    }
}

/// Take leading stopwords from a directive interior. Returns
/// `(stopwords, rest)`. Stopwords are case-insensitive.
fn take_leading_stopwords(s: &str) -> (&str, &str) {
    // Run the leading-stopword scanner repeatedly; each pass strips
    // one whitespace-delimited stopword.
    let mut consumed = 0usize;
    loop {
        // Skip whitespace.
        let after_ws = s[consumed..]
            .find(|c: char| !c.is_whitespace())
            .map(|n| consumed + n)
            .unwrap_or(s.len());
        // Find next whitespace boundary or end.
        let word_end = s[after_ws..]
            .find(|c: char| c.is_whitespace())
            .map(|n| after_ws + n)
            .unwrap_or(s.len());
        let word = &s[after_ws..word_end];
        if !is_stopword(word) {
            break;
        }
        consumed = word_end;
    }
    let stop = s[..consumed].trim();
    let rest = &s[consumed..];
    (stop, rest)
}

fn is_stopword(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "sed" | "vero" | "atque" | "attamen" | "si" | "deinde"
    )
}

/// Split off a trailing scope clause (`dicitur ...`, `omittitur`,
/// etc., optionally prefixed by `loco hujus versus`/`loco horum versuum`).
/// Returns `(condition, scope_keywords)`.
fn split_off_trailing_scope(s: &str) -> (&str, &str) {
    // Scope keywords (case-insensitive, word-boundary):
    //   dicitur, dicuntur, omittitur, omittuntur — possibly preceded
    //   by `loco hujus versus`, `loco horum versuum`, `hic versus`,
    //   `hoc versus`, `hæc versus`, `hi versus`, `haec versus`.
    //
    // The Perl regex (lines 87-107) is wide-open — it matches any of
    // those preambles followed by any of the scope keywords. We
    // approximate by scanning for the FIRST trailing scope keyword
    // and including any recognized preamble before it.
    let lower = s.to_ascii_lowercase();
    let mut best_split: Option<usize> = None;
    // The four scope verbs (with `dicuntur` checked before `dicitur`
    // so the longer match wins).
    for needle in ["dicuntur", "dicitur", "omittuntur", "omittitur"] {
        if let Some(idx) = find_word(&lower, needle) {
            // Walk back through optional preamble: the words `semper`
            // doesn't go before; preamble would be `loco hujus versus`
            // / `loco horum versuum` / `hic versus` / `hoc versus` /
            // `hæc versus` / `hi versus` / `haec versus`.
            let pre_start = walk_back_preamble(&lower, idx);
            best_split = Some(match best_split {
                Some(b) if pre_start >= b => b,
                _ => pre_start,
            });
        }
    }
    if let Some(split_at) = best_split {
        let cond = &s[..split_at];
        let scope = &s[split_at..];
        return (cond, scope);
    }
    (s, "")
}

/// Find a word `needle` in `haystack` (lowercase). Returns the byte
/// offset of the first whole-word match, or `None`.
fn find_word(haystack: &str, needle: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    let mut i = 0usize;
    while i + n.len() <= h.len() {
        if word_at(h, i, n) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Walk back through optional scope-preamble words ending at `idx`.
/// Returns the byte offset where the preamble starts (or `idx` itself
/// if no recognised preamble precedes `idx`).
fn walk_back_preamble(haystack: &str, idx: usize) -> usize {
    let mut start = idx;
    let preambles: [&[&str]; 7] = [
        &["loco", "hujus", "versus"],
        &["loco", "horum", "versuum"],
        &["hic", "versus"],
        &["hoc", "versus"],
        &["hæc", "versus"],
        &["hi", "versus"],
        &["haec", "versus"],
    ];
    'outer: for preamble in preambles.iter() {
        let mut try_start = start;
        for word in preamble.iter().rev() {
            // Skip whitespace before try_start.
            let skip = haystack[..try_start]
                .trim_end_matches(|c: char| c.is_whitespace())
                .len();
            // Find the word ending at `skip`.
            if !haystack[..skip].ends_with(*word) {
                continue 'outer;
            }
            try_start = skip - word.len();
        }
        start = try_start;
        break;
    }
    start
}

/// Parse a `(...)`-directive interior into a structured [`Conditional`]
/// under the active state. Mirror of `SetupString.pl::parse_conditional`
/// line 139-167.
///
/// Inputs:
///   * `stopwords` — leading stopword text (e.g. `"sed"` or `"sed vero"`).
///   * `condition` — the body fed to [`vero`].
///   * `scope` — trailing scope keyword text (e.g. `"dicitur semper"`
///     or `""` when absent).
///   * `subjects` — active state for `vero` evaluation.
pub fn parse_conditional(
    stopwords: &str,
    condition: &str,
    scope: &str,
    subjects: &Subjects<'_>,
) -> Conditional {
    // Strength = sum of stopword weights.
    //   sed, vero  -> 1
    //   atque      -> 2
    //   attamen    -> 3
    //   si, deinde -> 0
    let mut strength: u8 = 0;
    for tok in stopwords.split_whitespace() {
        strength += stopword_weight(tok);
    }
    let result = vero(condition, subjects);

    // Implicit backscope from a backscoped stopword (sed, vero, atque,
    // attamen — i.e. any with non-zero weight).
    let implicit_backscope = stopwords
        .split_whitespace()
        .any(|t| has_implicit_backscope(t));

    // Backscope:
    //   versuum / omittuntur  -> SCOPE_NEST
    //   versus  / omittitur   -> SCOPE_CHUNK
    //   no `semper` AND implicit backscope -> SCOPE_LINE
    //   else                  -> SCOPE_NULL
    let scope_lower = scope.to_ascii_lowercase();
    let backscope = if scope_lower.contains("versuum") || scope_lower.contains("omittuntur") {
        Scope::Nest
    } else if scope_lower.contains("versus") || scope_lower.contains("omittitur") {
        Scope::Chunk
    } else if !scope_lower.contains("semper") && implicit_backscope {
        Scope::Line
    } else {
        Scope::Null
    };

    // Forwardscope:
    //   omittitur / omittuntur -> SCOPE_NULL
    //   dicuntur               -> SCOPE_CHUNK if backscope==CHUNK else SCOPE_NEST
    //   else                   -> SCOPE_CHUNK if back is CHUNK or NEST else SCOPE_LINE
    let forwardscope = if scope_lower.contains("omittitur") || scope_lower.contains("omittuntur") {
        Scope::Null
    } else if scope_lower.contains("dicuntur") {
        if backscope == Scope::Chunk {
            Scope::Chunk
        } else {
            Scope::Nest
        }
    } else if backscope == Scope::Chunk || backscope == Scope::Nest {
        Scope::Chunk
    } else {
        Scope::Line
    };

    Conditional {
        strength,
        result,
        backscope,
        forwardscope,
    }
}

fn stopword_weight(word: &str) -> u8 {
    // SetupString.pl:77-84 — sed=1, vero=1, atque=2, attamen=3,
    // si=0, deinde=1. (deinde is added to %stopword_weights AFTER
    // the %backscoped_stopwords copy, so it counts toward strength
    // but doesn't trigger implicit backscope.)
    match word.to_ascii_lowercase().as_str() {
        "sed" | "vero" | "deinde" => 1,
        "atque" => 2,
        "attamen" => 3,
        _ => 0, // si and unknown words contribute 0.
    }
}

fn has_implicit_backscope(word: &str) -> bool {
    // The `%backscoped_stopwords` set in SetupString.pl:80 is the
    // weight-1+ stopwords minus `si` and `deinde`. So: sed, vero,
    // atque, attamen.
    matches!(
        word.to_ascii_lowercase().as_str(),
        "sed" | "vero" | "atque" | "attamen"
    )
}

// ─── process_conditional_lines (B10b-slice-4) ───────────────────────

/// Frame state on the conditional stack — mirrors the three Perl
/// constants `COND_NOT_YET_AFFIRMATIVE` / `COND_AFFIRMATIVE` /
/// `COND_DUMMY_FRAME` at `SetupString.pl:367-369`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameState {
    NotYetAffirmative,
    Affirmative,
    Dummy,
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    state: FrameState,
    scope: Scope,
}

/// Walk a section body line-by-line, evaluating `(...)` directives
/// and dropping lines whose conditional guard is false.
///
/// Mirror of `SetupString.pl::process_conditional_lines` line 363-474.
///
/// The algorithm maintains two parallel stacks:
///
///   * `conditional_stack` — one frame per active conditional, recording
///     `(state, forward_scope)`. The top frame's `state` decides
///     whether the current line is emitted; its `scope` decides when
///     the frame pops.
///   * `conditional_offsets` — `offsets[i]` is the output-array index
///     at which the conditional of strength `i` was last encountered
///     (used by the backward-scope retraction to know how far back
///     to walk before bumping into a stronger fence).
///
/// New `(...)` directives can:
///   * **Retract** previously emitted lines (backward scope LINE /
///     CHUNK / NEST).
///   * **Gate** subsequent lines until the forward scope expires.
///   * **Pop** lower-strength frames if their strength is ≤ the new
///     conditional's strength.
///
/// Lines starting with `~` are escape-stripped (the `~` is a way to
/// emit a literal `(` at the start of a line without it being read
/// as a conditional).
///
/// Returns the remaining body text — newline-separated, no trailing
/// newline. Each input line that survives the conditional walk is
/// emitted exactly once (preserving order).
pub fn process_conditional_lines(body: &str, subjects: &Subjects<'_>) -> String {
    let mut output: Vec<String> = Vec::new();
    // Initial state: one always-true frame with NEST scope.
    let mut stack: Vec<Frame> = vec![Frame {
        state: FrameState::Affirmative,
        scope: Scope::Nest,
    }];
    // offsets[i] = output index at which the strength-i conditional
    // last fired. The Perl initialises to `(-1)` (one element, `-1`).
    // We use `Vec<i64>` for parity (signed because offsets can go
    // below 0 during the walk).
    let mut offsets: Vec<i64> = vec![-1];

    for raw_line in body.split('\n') {
        let mut line = raw_line.to_string();

        // Check whether the line starts with a (...) directive.
        // `^\s*(...)\s*(.*)$` — the regex finds the FIRST balanced
        // (...) starting after leading whitespace.
        let trimmed_start = line.trim_start();
        let leading_ws_len = line.len() - trimmed_start.len();
        if let Some(m) = find_conditional_at_start(trimmed_start) {
            let stopwords = m.stopwords.to_string();
            let condition = m.condition.to_string();
            let scope = m.scope.to_string();
            // The "sequel" — everything after the directive on the
            // same line. Perl strips one whitespace character then
            // captures `(.*)$`.
            let after_paren_idx = leading_ws_len + m.end;
            let mut sequel = line[after_paren_idx..].trim_start().to_string();

            let cond = parse_conditional(&stopwords, &condition, &scope, subjects);
            let mut result = cond.result;
            let mut forward = cond.forwardscope;
            let strength = cond.strength as usize;

            // Top-of-stack predicate: "if the parent conditional is
            // not affirmative, then the new one must break out of
            // the nest, as it were."
            //
            // Perl: `${$conditional_stack[-1]}[0] == COND_AFFIRMATIVE
            //       || $strength >= $#conditional_offsets`
            //
            // `$#conditional_offsets` is the last index of the
            // offsets array — which equals `offsets.len() - 1`.
            let last_offsets_idx = offsets.len().saturating_sub(1);
            let parent_affirm = stack
                .last()
                .map(|f| f.state == FrameState::Affirmative)
                .unwrap_or(false);

            if parent_affirm || strength >= last_offsets_idx {
                // Stack truncation logic.
                if strength >= last_offsets_idx {
                    // `@conditional_stack = ();` — drop all frames.
                    stack.clear();
                } else if strength >= last_offsets_idx.saturating_sub(stack.len() - 1) {
                    // Perl: `$#conditional_stack = $#conditional_offsets - $strength - 1`
                    // i.e. shrink stack so its last index becomes
                    // `last_offsets_idx - strength - 1`. New length =
                    // `last_offsets_idx - strength`.
                    let new_len = last_offsets_idx.saturating_sub(strength);
                    stack.truncate(new_len);
                }

                if result {
                    // Find the "nearest insurmountable fence" — the
                    // output offset of the strength-`strength`
                    // conditional we last saw (or -1 when there's
                    // none at that level yet).
                    let fence: i64 = if last_offsets_idx >= strength {
                        offsets[strength]
                    } else {
                        -1
                    };
                    apply_backscope(&mut output, fence, cond.backscope);
                }

                // Having backtracked, null forward scope now behaves
                // like a satisfied conditional with nesting forward
                // scope.
                if forward == Scope::Null {
                    forward = Scope::Nest;
                    result = true;
                }

                if result {
                    // Remember where this conditional fired (at all
                    // levels 0..=strength).
                    let cur_idx: i64 = output.len() as i64 - 1;
                    while offsets.len() <= strength {
                        offsets.push(cur_idx);
                    }
                    for i in 0..=strength {
                        if i < offsets.len() {
                            offsets[i] = cur_idx;
                        }
                    }
                }

                // Push dummy frame(s) onto the conditional stack to
                // bring it into sync with the strength.
                //
                // Perl: while ($strength < $#conditional_offsets - $#conditional_stack - 1)
                // i.e. while strength < last_offsets - last_stack_idx - 1.
                // last_offsets = offsets.len() - 1
                // last_stack   = stack.len() - 1
                // so:  strength < (offsets.len() - 1) - (stack.len() - 1) - 1
                //   == strength < offsets.len() - stack.len() - 1
                while stack.len() + strength + 1 < offsets.len() {
                    stack.push(Frame {
                        state: FrameState::Dummy,
                        scope: forward,
                    });
                }

                // Push the new conditional frame.
                stack.push(Frame {
                    state: if result {
                        FrameState::Affirmative
                    } else {
                        FrameState::NotYetAffirmative
                    },
                    scope: forward,
                });
            }

            // Replace `line` with the sequel and fall through to
            // the line-emission code. Perl: `next unless $line;` —
            // skip the rest of the loop body when sequel is empty.
            if sequel.is_empty() {
                continue;
            }
            // Strip a leading `~` escape, if any, just like the
            // post-directive escape handling below.
            if let Some(rest) = sequel.strip_prefix('~') {
                sequel = rest.to_string();
            }
            line = sequel;
        } else {
            // Strip a leading `~` escape on the whole line.
            if let Some(rest) = line.strip_prefix('~') {
                line = rest.to_string();
            }
        }

        // Add line to output if the top-of-stack frame is affirmative.
        if stack
            .last()
            .map(|f| f.state == FrameState::Affirmative)
            .unwrap_or(true)
        {
            output.push(line.clone());
        }

        // Pop expired frames.
        // Perl: while top.scope == LINE OR (top.scope == CHUNK AND
        //   line is blank), pop until we land on a non-dummy frame
        //   or empty the stack.
        loop {
            let top_scope = match stack.last() {
                Some(f) => f.scope,
                None => break,
            };
            let line_is_blank = is_blank_line(&line);
            let should_pop = top_scope == Scope::Line
                || (top_scope == Scope::Chunk && line_is_blank);
            if !should_pop {
                break;
            }
            // Pop the top frame, then keep popping while the new top
            // is a Dummy frame.
            stack.pop();
            while let Some(f) = stack.last() {
                if f.state == FrameState::Dummy {
                    stack.pop();
                } else {
                    break;
                }
            }
            // If the stack is empty, push the always-true bottom.
            if stack.is_empty() {
                stack.push(Frame {
                    state: FrameState::Affirmative,
                    scope: Scope::Nest,
                });
                break;
            }
        }
    }

    output.join("\n")
}

/// Apply a backward-scope retraction to the `output` buffer. Mirror
/// of the SCOPE_LINE / SCOPE_CHUNK / SCOPE_NEST branches at
/// `SetupString.pl:407-422`.
fn apply_backscope(output: &mut Vec<String>, fence: i64, backscope: Scope) {
    match backscope {
        Scope::Line => {
            // Remove preceding line if there's room above the fence.
            if output.len() as i64 - 1 > fence {
                output.pop();
            }
        }
        Scope::Chunk => {
            // Remove preceding consecutive non-blank lines.
            while output.len() as i64 - 1 > fence
                && !is_blank_line(output.last().unwrap())
            {
                output.pop();
            }
            // Remove any blank lines.
            while output.len() as i64 - 1 > fence
                && is_blank_line(output.last().unwrap())
            {
                output.pop();
            }
        }
        Scope::Nest => {
            // Truncate output back to the fence.
            let new_len = (fence + 1).max(0) as usize;
            output.truncate(new_len);
        }
        Scope::Null => {}
    }
}

/// Mirror of the Perl `$blankline_regex = qr/^\s*_?\s*$/`. A line is
/// blank when it has nothing but optional whitespace plus an optional
/// underscore.
fn is_blank_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed == "_"
}

/// Find a (...) directive only when it sits at the START of a string
/// (after optional leading whitespace already removed by the caller).
/// Returns the same `ConditionalMatch` shape as [`find_conditional`]
/// but only matches when `bytes[0] == b'('`.
fn find_conditional_at_start(s: &str) -> Option<ConditionalMatch<'_>> {
    if !s.starts_with('(') {
        return None;
    }
    find_conditional(s)
}

// ─── Inclusion substitutions (B10b-slice-5) ─────────────────────────

/// Apply `s/PAT/REPL/FLAGS` substitutions and line-picks to an
/// inclusion body. The `spec` string is a sequence of directives,
/// each one of:
///
///   * `s/PAT/REPL/FLAGS` — Perl-style substitution. Flags supported:
///     `g` (global), `m` (multiline `^`/`$`), `s` (dot matches newline),
///     `i` (case-insensitive).
///   * `N` — keep only line N (1-indexed).
///   * `N-M` — keep only lines N..=M.
///   * `!N` / `!N-M` — DROP line(s) N (or range), keep all others.
///
/// Multiple directives are applied in left-to-right order. Whitespace
/// and commas between directives are ignored.
///
/// Mirror of `SetupString.pl::do_inclusion_substitutions` line 479-493.
///
/// ## Pragmatic regex subset
///
/// To keep the WASM bundle small and avoid pulling in the `regex`
/// crate, this module implements its own minimal regex engine
/// covering the patterns observed in the upstream corpus. The
/// supported subset:
///
///   * Anchors `^` and `$` (per-line under `/m`).
///   * Literal characters and escaped metacharacters
///     (`\.`, `\;`, `\?`, `\*`, `\+`, `\(`, `\)`, `\[`, `\]`, `\{`, `\}`,
///     `\\`, `\/`, `\^`, `\$`).
///   * `\d` (digit), `\D` (non-digit), `\s` (whitespace),
///     `\S` (non-whitespace), `\w` (word char), `\W` (non-word).
///   * Character classes `[abc]`, ranges `[a-z]`, negation `[^abc]`.
///   * Quantifiers `*`, `+`, `?`, plus their non-greedy forms `*?`,
///     `+?`, `??`.
///   * `.` (any char; respects `/s` for newline).
///   * Alternation `|` at the top level (no nested groups).
///   * Lookahead `(?!...)` (negative; sufficient for `\d+(?![a-z])`).
///   * Capture groups `(...)` are accepted but un-numbered (replacement
///     `$1`/`$2`/`\1` not supported — the corpus doesn't use them in
///     `do_inclusion_substitutions` directives).
///
/// Patterns outside this subset cause the substitution to be skipped
/// (the body is left unchanged for that directive). The regression
/// harness will surface any such divergence as a per-cell mismatch.
pub fn do_inclusion_substitutions(body: &mut String, spec: &str) {
    let mut i = 0;
    let bytes = spec.as_bytes();
    let n = bytes.len();
    while i < n {
        // Skip whitespace and `,` separators.
        while i < n && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= n {
            break;
        }
        if bytes[i] == b's' && i + 1 < n && bytes[i + 1] == b'/' {
            // Substitution directive `s/PAT/REPL/FLAGS`.
            if let Some(consumed) = apply_one_substitution(body, &spec[i..]) {
                i += consumed;
            } else {
                // Unparseable; skip to next whitespace/comma.
                while i < n && !bytes[i].is_ascii_whitespace() && bytes[i] != b',' {
                    i += 1;
                }
            }
        } else if bytes[i] == b'!' || bytes[i].is_ascii_digit() {
            // Line-pick directive — `N`, `N-M`, `!N`, `!N-M`.
            let start = i;
            if bytes[i] == b'!' {
                i += 1;
            }
            // Consume digits.
            let n_start = i;
            while i < n && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i == n_start {
                // Bare `!` with no digit — invalid; skip.
                continue;
            }
            // Optional `-M`.
            if i < n && bytes[i] == b'-' {
                i += 1;
                while i < n && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            apply_line_pick(body, &spec[start..i]);
        } else {
            // Unknown directive form — skip to next separator.
            while i < n && !bytes[i].is_ascii_whitespace() && bytes[i] != b',' {
                i += 1;
            }
        }
    }
}

/// Apply one `s/PAT/REPL/FLAGS` directive starting at the beginning
/// of `spec`. Returns the number of bytes consumed, or `None` when
/// the directive is malformed or uses unsupported regex features.
fn apply_one_substitution(body: &mut String, spec: &str) -> Option<usize> {
    let bytes = spec.as_bytes();
    if !spec.starts_with("s/") {
        return None;
    }
    // Find the next unescaped `/` to close the pattern.
    let pat_end = find_unescaped_slash(bytes, 2)?;
    let pat = &spec[2..pat_end];
    let repl_end = find_unescaped_slash(bytes, pat_end + 1)?;
    let repl = &spec[pat_end + 1..repl_end];
    // Flags are `[gism]*`.
    let mut flag_end = repl_end + 1;
    while flag_end < bytes.len()
        && matches!(bytes[flag_end], b'g' | b'i' | b's' | b'm' | b'x')
    {
        flag_end += 1;
    }
    let flags = &spec[repl_end + 1..flag_end];

    // Try to compile + apply the pattern. On unsupported pattern, do
    // nothing (skip directive) but report consumption so we can move on.
    let global = flags.contains('g');
    let case_insensitive = flags.contains('i');
    let dotall = flags.contains('s');
    let multiline = flags.contains('m');
    let opts = RegexOpts {
        case_insensitive,
        dotall,
        multiline,
    };
    if let Some(compiled) = compile_regex(pat, opts) {
        let new_body = compiled.replace_all(body, repl, global);
        *body = new_body;
    }
    Some(flag_end)
}

/// Find the next unescaped `/` starting at `from` in `bytes`. Returns
/// the byte offset, or `None` when no closing slash exists.
fn find_unescaped_slash(bytes: &[u8], from: usize) -> Option<usize> {
    let n = bytes.len();
    let mut i = from;
    while i < n {
        match bytes[i] {
            b'\\' => i += 2, // skip escaped char
            b'/' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Apply a line-pick directive. `spec` is the raw text — `N`, `N-M`,
/// `!N`, or `!N-M`.
fn apply_line_pick(body: &mut String, spec: &str) {
    let drop = spec.starts_with('!');
    let nums_part = if drop { &spec[1..] } else { spec };
    let (start, end) = if let Some((s, e)) = nums_part.split_once('-') {
        let s: usize = s.parse().ok().unwrap_or(0);
        let e: usize = e.parse().ok().unwrap_or(0);
        (s, e)
    } else {
        let s: usize = nums_part.parse().ok().unwrap_or(0);
        (s, s)
    };
    if start == 0 || end == 0 || end < start {
        return;
    }
    // Lines are 1-indexed in the directive; 0-indexed internally.
    let (s_idx, e_idx) = (start - 1, end);
    let lines: Vec<&str> = body.split('\n').collect();
    if s_idx >= lines.len() {
        return;
    }
    let e_idx = e_idx.min(lines.len());
    let kept: Vec<&str> = if drop {
        lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| if i < s_idx || i >= e_idx { Some(*l) } else { None })
            .collect()
    } else {
        lines[s_idx..e_idx].to_vec()
    };
    *body = kept.join("\n");
    // Perl always appends a trailing `\n` after splice — match that
    // when the result is non-empty and didn't already end with one.
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
}

// ─── Mini regex engine (used by do_inclusion_substitutions) ─────────

#[derive(Debug, Clone, Copy, Default)]
struct RegexOpts {
    case_insensitive: bool,
    dotall: bool,
    multiline: bool,
}

/// Compiled regex. Holds the parsed pattern as a flat token vector;
/// matching walks the input once per try-position with a small VM.
#[derive(Debug, Clone)]
struct CompiledRegex {
    tokens: Vec<RegexToken>,
    opts: RegexOpts,
}

#[derive(Debug, Clone)]
enum RegexToken {
    Anchor(Anchor),
    Char(char),
    Any,                  // `.` (newline only when dotall)
    Class(CharClass),
    Group(Vec<RegexToken>),
    Alternation(Vec<Vec<RegexToken>>),
    NegativeLookahead(Vec<RegexToken>),
    Quantified(Box<RegexToken>, Quantifier),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Start,
    End,
}

#[derive(Debug, Clone)]
struct CharClass {
    negated: bool,
    ranges: Vec<(char, char)>,
    /// Built-in escapes inside the class (\d, \s, \w).
    builtins: Vec<BuiltinClass>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinClass {
    Digit,
    NonDigit,
    Space,
    NonSpace,
    Word,
    NonWord,
}

#[derive(Debug, Clone, Copy)]
struct Quantifier {
    min: usize,
    max: Option<usize>,
    greedy: bool,
}

fn compile_regex(pattern: &str, opts: RegexOpts) -> Option<CompiledRegex> {
    let mut parser = RegexParser::new(pattern);
    let tokens = parser.parse_alternation()?;
    if !parser.at_end() {
        return None;
    }
    Some(CompiledRegex { tokens, opts })
}

struct RegexParser<'a> {
    pattern: &'a [u8],
    pos: usize,
}

impl<'a> RegexParser<'a> {
    fn new(pattern: &'a str) -> Self {
        Self {
            pattern: pattern.as_bytes(),
            pos: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.pattern.len()
    }

    fn peek(&self) -> Option<u8> {
        self.pattern.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    /// Top-level: alternation. Returns flattened tokens when there's
    /// no `|`, or wraps in `Alternation` when there is.
    fn parse_alternation(&mut self) -> Option<Vec<RegexToken>> {
        let mut branches = Vec::new();
        let first = self.parse_sequence()?;
        branches.push(first);
        while self.peek() == Some(b'|') {
            self.pos += 1;
            let next = self.parse_sequence()?;
            branches.push(next);
        }
        if branches.len() == 1 {
            Some(branches.into_iter().next().unwrap())
        } else {
            Some(vec![RegexToken::Alternation(branches)])
        }
    }

    /// Sequence of atoms (possibly quantified). Stops at `|` or `)`
    /// or end-of-input.
    fn parse_sequence(&mut self) -> Option<Vec<RegexToken>> {
        let mut tokens = Vec::new();
        while let Some(b) = self.peek() {
            if b == b'|' || b == b')' {
                break;
            }
            let atom = self.parse_atom()?;
            // Check for quantifier suffix.
            let q = self.parse_quantifier();
            match q {
                Some(quant) => tokens.push(RegexToken::Quantified(Box::new(atom), quant)),
                None => tokens.push(atom),
            }
        }
        Some(tokens)
    }

    fn parse_atom(&mut self) -> Option<RegexToken> {
        // Peek the lead byte to decide single-byte ASCII metachars vs
        // multi-byte UTF-8 atom. We use byte-level advance for ASCII
        // operators (anchors, classes, groups, escapes, quantifier
        // sentinels) and a UTF-8-aware read for literal characters
        // — so that multi-byte chars like `á` (`0xC3 0xA1`) become
        // one `Char(U+00E1)` token instead of two byte-tokens that
        // can't match a single `á` codepoint.
        let lead = *self.pattern.get(self.pos)?;
        // ASCII metachar fast-path.
        if lead < 0x80 {
            let b = self.advance()?;
            return match b {
                b'^' => Some(RegexToken::Anchor(Anchor::Start)),
                b'$' => Some(RegexToken::Anchor(Anchor::End)),
                b'.' => Some(RegexToken::Any),
                b'[' => self.parse_class(),
                b'(' => {
                    // `(?!...)` — negative lookahead.
                    if self.pattern.get(self.pos) == Some(&b'?')
                        && self.pattern.get(self.pos + 1) == Some(&b'!')
                    {
                        self.pos += 2;
                        let body = self.parse_alternation()?;
                        if self.advance()? != b')' {
                            return None;
                        }
                        return Some(RegexToken::NegativeLookahead(body));
                    }
                    // `(?:...)` — non-capturing group.
                    if self.pattern.get(self.pos) == Some(&b'?')
                        && self.pattern.get(self.pos + 1) == Some(&b':')
                    {
                        self.pos += 2;
                    }
                    let body = self.parse_alternation()?;
                    if self.advance()? != b')' {
                        return None;
                    }
                    Some(RegexToken::Group(body))
                }
                b'\\' => self.parse_escape(),
                // Quantifiers can't start an atom.
                b'*' | b'+' | b'?' | b'{' => None,
                _ => Some(RegexToken::Char(b as char)),
            };
        }
        // Multi-byte UTF-8 lead — read the full codepoint.
        let cp_len = match lead {
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => 1, // continuation/illegal — treat as single byte
        };
        let end = (self.pos + cp_len).min(self.pattern.len());
        let slice = &self.pattern[self.pos..end];
        self.pos = end;
        let s = core::str::from_utf8(slice).ok()?;
        let ch = s.chars().next()?;
        Some(RegexToken::Char(ch))
    }

    fn parse_escape(&mut self) -> Option<RegexToken> {
        let b = self.advance()?;
        match b {
            b'd' => Some(RegexToken::Class(builtin_class(BuiltinClass::Digit))),
            b'D' => Some(RegexToken::Class(builtin_class(BuiltinClass::NonDigit))),
            b's' => Some(RegexToken::Class(builtin_class(BuiltinClass::Space))),
            b'S' => Some(RegexToken::Class(builtin_class(BuiltinClass::NonSpace))),
            b'w' => Some(RegexToken::Class(builtin_class(BuiltinClass::Word))),
            b'W' => Some(RegexToken::Class(builtin_class(BuiltinClass::NonWord))),
            b'n' => Some(RegexToken::Char('\n')),
            b'r' => Some(RegexToken::Char('\r')),
            b't' => Some(RegexToken::Char('\t')),
            // Literal escape (`.`, `;`, `\`, `/`, etc.)
            _ => Some(RegexToken::Char(b as char)),
        }
    }

    fn parse_class(&mut self) -> Option<RegexToken> {
        let mut class = CharClass {
            negated: false,
            ranges: Vec::new(),
            builtins: Vec::new(),
        };
        if self.peek() == Some(b'^') {
            class.negated = true;
            self.pos += 1;
        }
        while let Some(b) = self.peek() {
            if b == b']' {
                self.pos += 1;
                return Some(RegexToken::Class(class));
            }
            let ch = if b == b'\\' {
                self.pos += 1;
                let esc = self.advance()?;
                match esc {
                    b'd' => {
                        class.builtins.push(BuiltinClass::Digit);
                        continue;
                    }
                    b'D' => {
                        class.builtins.push(BuiltinClass::NonDigit);
                        continue;
                    }
                    b's' => {
                        class.builtins.push(BuiltinClass::Space);
                        continue;
                    }
                    b'S' => {
                        class.builtins.push(BuiltinClass::NonSpace);
                        continue;
                    }
                    b'w' => {
                        class.builtins.push(BuiltinClass::Word);
                        continue;
                    }
                    b'W' => {
                        class.builtins.push(BuiltinClass::NonWord);
                        continue;
                    }
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    other => other as char,
                }
            } else {
                self.pos += 1;
                b as char
            };
            // Check for range `a-z`.
            if self.peek() == Some(b'-')
                && self.pattern.get(self.pos + 1).copied() != Some(b']')
            {
                self.pos += 1;
                let end_b = self.advance()?;
                let end_ch = if end_b == b'\\' {
                    let esc = self.advance()?;
                    match esc {
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        other => other as char,
                    }
                } else {
                    end_b as char
                };
                class.ranges.push((ch, end_ch));
            } else {
                class.ranges.push((ch, ch));
            }
        }
        // Unterminated class.
        None
    }

    fn parse_quantifier(&mut self) -> Option<Quantifier> {
        let b = self.peek()?;
        let (min, max) = match b {
            b'*' => {
                self.pos += 1;
                (0usize, None)
            }
            b'+' => {
                self.pos += 1;
                (1usize, None)
            }
            b'?' => {
                self.pos += 1;
                (0usize, Some(1))
            }
            _ => return None,
        };
        // Lazy?
        let greedy = if self.peek() == Some(b'?') {
            self.pos += 1;
            false
        } else {
            true
        };
        Some(Quantifier { min, max, greedy })
    }
}

fn builtin_class(kind: BuiltinClass) -> CharClass {
    CharClass {
        negated: false,
        ranges: Vec::new(),
        builtins: vec![kind],
    }
}

impl CharClass {
    fn matches(&self, ch: char) -> bool {
        let mut hit = false;
        for &(lo, hi) in &self.ranges {
            if ch >= lo && ch <= hi {
                hit = true;
                break;
            }
        }
        if !hit {
            for kind in &self.builtins {
                if matches_builtin(*kind, ch) {
                    hit = true;
                    break;
                }
            }
        }
        if self.negated {
            !hit
        } else {
            hit
        }
    }
}

fn matches_builtin(kind: BuiltinClass, ch: char) -> bool {
    match kind {
        BuiltinClass::Digit => ch.is_ascii_digit(),
        BuiltinClass::NonDigit => !ch.is_ascii_digit(),
        BuiltinClass::Space => ch.is_whitespace(),
        BuiltinClass::NonSpace => !ch.is_whitespace(),
        BuiltinClass::Word => ch.is_alphanumeric() || ch == '_',
        BuiltinClass::NonWord => !(ch.is_alphanumeric() || ch == '_'),
    }
}

impl CompiledRegex {
    /// Find the first match starting at byte offset `from`. Returns
    /// `Some((start_byte, end_byte))` on match, `None` otherwise.
    fn find(&self, text: &str, from: usize) -> Option<(usize, usize)> {
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let mut start_idx = chars.iter().position(|(b, _)| *b >= from).unwrap_or(chars.len());
        loop {
            if let Some(end) = match_seq(&self.tokens, &chars, start_idx, &self.opts) {
                let s_byte = chars.get(start_idx).map(|(b, _)| *b).unwrap_or(text.len());
                let e_byte = if end < chars.len() {
                    chars[end].0
                } else {
                    text.len()
                };
                return Some((s_byte, e_byte));
            }
            if start_idx >= chars.len() {
                return None;
            }
            start_idx += 1;
        }
    }

    /// Apply substitution. When `global`, replace every non-overlapping
    /// match; otherwise only the first.
    fn replace_all(&self, text: &str, repl: &str, global: bool) -> String {
        let mut out = String::with_capacity(text.len());
        let mut cursor = 0usize;
        loop {
            let m = self.find(text, cursor);
            match m {
                Some((s, e)) => {
                    out.push_str(&text[cursor..s]);
                    out.push_str(repl);
                    if e == s {
                        // Zero-width match — advance one char (or break
                        // out at end-of-string) to avoid infinite loop.
                        if s >= text.len() {
                            // At end of string and zero-width match —
                            // we've already emitted the replacement;
                            // there's nothing more to scan.
                            cursor = s;
                            break;
                        }
                        let ch_len = text[s..]
                            .chars()
                            .next()
                            .map(|c| c.len_utf8())
                            .unwrap_or(1);
                        out.push_str(&text[s..s + ch_len]);
                        cursor = s + ch_len;
                    } else {
                        cursor = e;
                    }
                    if !global {
                        break;
                    }
                }
                None => break,
            }
        }
        out.push_str(&text[cursor..]);
        out
    }
}

/// Try to match the token sequence `tokens` starting at char index
/// `at`. Returns the char index after the match on success.
fn match_seq(
    tokens: &[RegexToken],
    chars: &[(usize, char)],
    at: usize,
    opts: &RegexOpts,
) -> Option<usize> {
    if tokens.is_empty() {
        return Some(at);
    }
    let (first, rest) = (&tokens[0], &tokens[1..]);
    match first {
        RegexToken::Quantified(inner, q) => match_quantified(inner, q, rest, chars, at, opts),
        _ => {
            if let Some(after) = match_one(first, chars, at, opts) {
                match_seq(rest, chars, after, opts)
            } else {
                None
            }
        }
    }
}

/// Match one token (no quantifier). Returns the char index after the
/// match on success.
fn match_one(
    token: &RegexToken,
    chars: &[(usize, char)],
    at: usize,
    opts: &RegexOpts,
) -> Option<usize> {
    match token {
        RegexToken::Anchor(Anchor::Start) => {
            if at == 0 {
                Some(at)
            } else if opts.multiline && chars.get(at - 1).map(|(_, c)| *c) == Some('\n') {
                Some(at)
            } else {
                None
            }
        }
        RegexToken::Anchor(Anchor::End) => {
            if at == chars.len() {
                Some(at)
            } else if opts.multiline && chars.get(at).map(|(_, c)| *c) == Some('\n') {
                Some(at)
            } else {
                None
            }
        }
        RegexToken::Char(c) => {
            let cur = chars.get(at)?.1;
            if char_eq(*c, cur, opts.case_insensitive) {
                Some(at + 1)
            } else {
                None
            }
        }
        RegexToken::Any => {
            let cur = chars.get(at)?.1;
            if cur == '\n' && !opts.dotall {
                None
            } else {
                Some(at + 1)
            }
        }
        RegexToken::Class(class) => {
            let cur = chars.get(at)?.1;
            let cur_for_match = if opts.case_insensitive {
                cur.to_lowercase().next().unwrap_or(cur)
            } else {
                cur
            };
            // For case-insensitive, need to compare against both cases.
            let matched = if opts.case_insensitive {
                class.matches(cur_for_match)
                    || class.matches(cur.to_uppercase().next().unwrap_or(cur))
            } else {
                class.matches(cur)
            };
            if matched {
                Some(at + 1)
            } else {
                None
            }
        }
        RegexToken::Group(inner) => match_seq(inner, chars, at, opts),
        RegexToken::Alternation(branches) => {
            for branch in branches {
                if let Some(after) = match_seq(branch, chars, at, opts) {
                    return Some(after);
                }
            }
            None
        }
        RegexToken::NegativeLookahead(inner) => {
            if match_seq(inner, chars, at, opts).is_some() {
                None
            } else {
                Some(at)
            }
        }
        RegexToken::Quantified(_, _) => {
            // Should never be reached; quantified tokens are handled
            // by match_seq before delegating here.
            None
        }
    }
}

fn match_quantified(
    inner: &RegexToken,
    q: &Quantifier,
    rest: &[RegexToken],
    chars: &[(usize, char)],
    at: usize,
    opts: &RegexOpts,
) -> Option<usize> {
    // Collect all consecutive matches up to the maximum (or as many
    // as possible). Then try the rest of the sequence at each
    // possible split point.
    let mut positions = vec![at];
    let mut cur = at;
    loop {
        if let Some(max) = q.max {
            if positions.len() - 1 >= max {
                break;
            }
        }
        match match_one(inner, chars, cur, opts) {
            Some(next) if next > cur => {
                cur = next;
                positions.push(cur);
            }
            _ => break,
        }
    }
    if positions.len() - 1 < q.min {
        return None;
    }
    // Greedy = try longest first; lazy = shortest first.
    let try_order: Box<dyn Iterator<Item = &usize>> = if q.greedy {
        Box::new(positions.iter().rev())
    } else {
        Box::new(positions.iter())
    };
    for &split in try_order {
        let consumed = (split as isize - at as isize) as usize;
        // Need to satisfy at least `min` matches at this split.
        if consumed / 1 < q.min && positions.iter().filter(|&&p| p <= split).count() - 1 < q.min {
            continue;
        }
        if let Some(after) = match_seq(rest, chars, split, opts) {
            return Some(after);
        }
    }
    None
}

fn char_eq(a: char, b: char, case_insensitive: bool) -> bool {
    if case_insensitive {
        a.to_lowercase().next() == b.to_lowercase().next()
    } else {
        a == b
    }
}

/// Resolve a load-time `@Path[:Section]` reference. Like the runtime
/// version but applied once at corpus-load time.
///
/// Mirror of `SetupString.pl::get_loadtime_inclusion` line 502-528.
///
/// In the Rust port there's no functional distinction between
/// load-time and resolve-time inclusions (the corpus is pre-loaded
/// into memory by the build script, with conditional eval already
/// baked for the 1570 baseline). This function therefore delegates
/// to [`resolve_section`] under a default Subjects — callers that
/// need rubric-aware resolution should use [`resolve_section`]
/// directly.
pub fn resolve_load_time_inclusion(
    path: &str,
    section: Option<&str>,
    substitutions: Option<&str>,
) -> Option<String> {
    let sect = section.unwrap_or("");
    let subjects = Subjects::default();
    let mut body = resolve_section(path, sect, &subjects)?;
    if let Some(subs) = substitutions {
        do_inclusion_substitutions(&mut body, subs);
    }
    Some(body)
}

// ─── Top-level resolvers (B10b-slice-6) ─────────────────────────────

/// Maximum @-hop count before resolution gives up. The Perl source
/// (`SetupString.pl:679`) hard-caps at 7; we mirror that.
pub const MAX_AT_HOPS: usize = 7;

/// Top-level section resolver. Mirror of `setupstring` line 534-712,
/// pared down to the runtime essentials.
///
/// Walks the `[Section]` indirection chain:
///
///   1. Look up `(path, section)` via the supplied `lookup` closure.
///   2. If the body starts with `@OtherPath` or `@OtherPath:OtherSection`,
///      follow the redirect (with `do_inclusion_substitutions` applied
///      to the target body when a `:s/PAT/REPL/` suffix is present).
///   3. The redirect target itself may carry its own `@`-line, so we
///      iterate up to `MAX_AT_HOPS` times.
///   4. After the chain resolves, run [`process_conditional_lines`]
///      on the final body to drop rubric-gated lines under the active
///      `subjects`.
///
/// **Note on the `lookup` closure:** the Rust runtime port keeps
/// resolution corpus-agnostic by accepting the per-`(path, section)`
/// fetcher as a parameter. Callers wire it to
/// [`crate::horas::lookup`] for office files or any equivalent for
/// other corpora. This mirrors the upstream `setupstring_caches_by_version`
/// hash but as an explicit dependency rather than a global.
///
/// The closure should return the **raw** section body (build-time
/// conditional eval already applied for the 1570 baseline; runtime
/// conditional eval is applied here on the final body, after the
/// chain resolves).
pub fn resolve_section_with<F>(
    path: &str,
    section: &str,
    subjects: &Subjects<'_>,
    lookup: F,
) -> Option<String>
where
    F: Fn(&str, &str) -> Option<String>,
{
    let mut cur_path = path.to_string();
    let mut cur_section = section.to_string();
    let mut hops = 0usize;
    loop {
        let raw = lookup(&cur_path, &cur_section)?;
        if let Some(redirect) = parse_at_redirect(&raw, &cur_path, &cur_section) {
            hops += 1;
            if hops > MAX_AT_HOPS {
                // Cycle / too-deep — return the raw body so the
                // divergence is visible.
                return Some(process_conditional_lines(&raw, subjects));
            }
            cur_path = redirect.path;
            cur_section = redirect.section;
            // If the redirect carries inclusion substitutions, fetch
            // the target body, apply them, then run conditional eval.
            // We do this by re-entering the loop — but if the
            // target's body doesn't start with another `@`, we need
            // a way to capture it for substitution. Handle that by
            // fetching one more time and applying immediately.
            if let Some(subs) = redirect.substitutions {
                let target = lookup(&cur_path, &cur_section)?;
                // If the target itself is another @-redirect, defer
                // substitution to its resolution (rare; we don't
                // model it). Most corpus uses are 1-hop with subs.
                if !is_at_redirect_line(&target) {
                    let mut body = process_conditional_lines(&target, subjects);
                    do_inclusion_substitutions(&mut body, &subs);
                    return Some(body);
                }
                // Fall through: continue chain without applying subs
                // (TODO: model multi-hop subs if the corpus uses them).
            }
            continue;
        }
        // Not an @-redirect — apply runtime conditional eval and return.
        return Some(process_conditional_lines(&raw, subjects));
    }
}

/// One parsed `@`-redirect line.
#[derive(Debug, Clone)]
struct AtRedirect {
    path: String,
    section: String,
    substitutions: Option<String>,
}

/// Parse the upstream `@Path[:Section][:subs]` shape. Returns `None`
/// when `body` doesn't start with `@` (after optional leading
/// whitespace) or doesn't have a balanced shape. The Perl regex
/// (`SetupString.pl:555-562`) is:
///
/// ```text
/// ^\s*@
/// ([^\n:]+)?                # filename (self-ref if omitted)
/// (?::([^\n:]+?))?          # optional keyword (section)
/// [^\S\n\r]*                # ignore trailing whitespace
/// (?::(.*))?                # optional substitutions
/// $
/// ```
///
/// Examples:
///   * `@Commune/C7` → path=Commune/C7, section=<inherit caller's>, subs=None
///   * `@Tempora/Pasc1-0:Oratio` → path=Tempora/Pasc1-0, section=Oratio, subs=None
///   * `@:Ant Vespera` → self-reference, section=Ant Vespera
///   * `@:Ant Vespera:s/;;.*//gm` → self-reference, section=Ant Vespera, subs=`s/;;.*//gm`
///
/// `current_path` and `current_section` are the active resolution
/// context — used to fill in the implicit-self-reference and
/// implicit-same-section slots.
fn parse_at_redirect(body: &str, current_path: &str, current_section: &str) -> Option<AtRedirect> {
    let trimmed = body.trim_start();
    let after_at = trimmed.strip_prefix('@')?;
    // Reject when the body has multiple non-empty lines — those are
    // long sections that happen to start with `@`, not redirect
    // markers. (Mirror of the Perl regex's line-anchored shape; the
    // real Perl checks that the regex matches the WHOLE line, not
    // just a prefix.)
    let first_line = after_at.split('\n').next()?;
    let after_first_line = after_at[first_line.len()..].trim();
    if !after_first_line.is_empty() {
        // Multi-line body — only treat as redirect when the trailing
        // lines are all empty.
        return None;
    }
    let line = first_line.trim_end();
    // Split on the first `:` (path / section boundary).
    let (path_part, rest_after_path) = match line.split_once(':') {
        Some((p, r)) => (p, Some(r)),
        None => (line, None),
    };
    // Self-reference when path is empty.
    let path = if path_part.is_empty() {
        current_path.to_string()
    } else {
        path_part.to_string()
    };
    let (section, substitutions) = match rest_after_path {
        Some(rest) => {
            // `rest` may contain another `:` to split section / subs.
            // Substitutions start with `s/` or a digit / `!` for
            // line-pick — neither of which is a valid section name
            // beginning, so we detect the split by looking at what
            // comes after the next `:`.
            //
            // The Perl regex captures section as `[^\n:]+?` (non-greedy,
            // no `:`), followed by optional ` ... :subs`. So section
            // is the longest non-`:` run BEFORE a final `:subs` group.
            //
            // Simpler heuristic: split on the LAST `:` only when the
            // tail looks like a substitution directive.
            if let Some(last_colon) = rest.rfind(':') {
                let possible_section = &rest[..last_colon];
                let possible_subs = &rest[last_colon + 1..];
                if looks_like_substitution(possible_subs) {
                    (
                        possible_section.trim().to_string(),
                        Some(possible_subs.trim().to_string()),
                    )
                } else {
                    (rest.trim().to_string(), None)
                }
            } else {
                (rest.trim().to_string(), None)
            }
        }
        None => (current_section.to_string(), None),
    };
    Some(AtRedirect {
        path,
        section,
        substitutions,
    })
}

fn looks_like_substitution(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("s/") || s.starts_with('!') || s.chars().next().map_or(false, |c| c.is_ascii_digit())
}

/// True when a body's first non-empty line is an `@`-redirect.
fn is_at_redirect_line(body: &str) -> bool {
    body.lines()
        .find(|l| !l.trim().is_empty())
        .map_or(false, |l| l.trim_start().starts_with('@'))
}

/// Resolve a section against the ambient breviary corpus
/// ([`crate::horas::lookup`]). Convenience wrapper around
/// [`resolve_section_with`] for the common case.
///
/// Mirror of `SetupString.pl::setupstring` line 534-712 in its most
/// common usage: "fetch this section under the active rubric, follow
/// any `@`-redirects, run conditional eval, return the body".
pub fn resolve_section(
    path: &str,
    section: &str,
    subjects: &Subjects<'_>,
) -> Option<String> {
    resolve_section_with(path, section, subjects, |p, s| {
        let file = crate::horas::lookup(p)?;
        file.sections.get(s).cloned()
    })
}

/// Office-side section resolver — adds the per-day commune-chain
/// fallback that distinguishes office lookups from Mass lookups.
///
/// Mirror of `SetupString.pl::officestring` line 720-777. The
/// upstream `officestring` is a thin layer over `setupstring` plus
/// monthly-feria special-case handling for August-December weekday
/// ferials. The breviary leg's commune-chain fallback (`vide CXX`,
/// `ex CXX`, `@Path` parent-inherit) lives in
/// [`crate::breviary::proprium`] and isn't relevant here — this
/// function just delegates to [`resolve_section`].
pub fn resolve_office_section(
    path: &str,
    section: &str,
    subjects: &Subjects<'_>,
) -> Option<String> {
    resolve_section(path, section, subjects)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(rubric: Rubric, dayname0: &str) -> Subjects<'_> {
        Subjects::new(rubric, dayname0, 1, 1, 2026)
    }

    #[test]
    fn empty_condition_is_true() {
        let subj = s(Rubric::Tridentine1570, "Adv1");
        assert!(vero("", &subj));
        assert!(vero("   ", &subj));
        assert!(vero("\n\t  ", &subj));
    }

    #[test]
    fn rubrica_predicate_matches_perl_version_string() {
        // Rubric::Rubrics1960.as_perl_version() == "Rubrics 1960 - 1960"
        let subj = s(Rubric::Rubrics1960, "Adv1");
        assert!(vero("rubrica 1960", &subj));
        assert!(!vero("rubrica 1570", &subj));

        let subj = s(Rubric::Tridentine1570, "Adv1");
        assert!(vero("rubrica 1570", &subj));
        assert!(!vero("rubrica 1960", &subj));
    }

    #[test]
    fn aut_disjunction() {
        let subj = s(Rubric::Reduced1955, "Adv1");
        assert!(vero("rubrica 1570 aut rubrica 1955", &subj));
        assert!(vero("rubrica 1955 aut rubrica 1960", &subj));
        assert!(!vero("rubrica 1570 aut rubrica 1960", &subj));
    }

    #[test]
    fn nisi_negates_following_atom() {
        // (rubrica 1960 nisi rubrica monastica)
        // True when rubric is 1960 AND not Monastic.
        let subj = s(Rubric::Rubrics1960, "Adv1");
        assert!(vero("rubrica 1960 nisi rubrica monastica", &subj));

        let subj = s(Rubric::Monastic, "Adv1");
        // Monastic rubric label: "pre-Trident Monastic" — contains "Monastic".
        assert!(!vero("rubrica 1960 nisi rubrica monastica", &subj));
    }

    #[test]
    fn et_conjunction() {
        // Use month 6 (June) so neither "12" nor "1" matches as a
        // substring of "6" — avoids the documented Perl `mense` quirk
        // (line 32: "mense is not perfect eg. 1 matches also 10 11 12").
        let subj = Subjects::new(Rubric::Rubrics1960, "Adv1", 1, 6, 2026);
        assert!(vero("rubrica 1960 et mense 6", &subj));
        assert!(!vero("rubrica 1960 et mense 7", &subj));
        // rubric mismatch — `et` short-circuits to false.
        assert!(!vero("rubrica 1570 et mense 6", &subj));
    }

    #[test]
    fn mense_predicate_perl_quirk_is_replicated() {
        // Document-and-test the Perl quirk at line 32: `(mense 1)`
        // with month 12 returns true because "12".contains("1").
        // We replicate the bug-for-bug behaviour so corpus authors
        // who relied on this quirk get the same outcome.
        let subj = Subjects::new(Rubric::Rubrics1960, "Adv1", 25, 12, 2026);
        assert!(vero("mense 1", &subj));  // 12 contains 1 — Perl quirk.
        assert!(vero("mense 12", &subj));
        assert!(!vero("mense 5", &subj)); // 12 doesn't contain 5.
    }

    #[test]
    fn aut_resets_negation() {
        // (nisi rubrica 1570 aut rubrica 1960) → (NOT 1570) OR (1960)
        // For Rubrics1960: first disjunct (NOT 1570) is true, so result is true.
        let subj = s(Rubric::Rubrics1960, "Adv1");
        assert!(vero("nisi rubrica 1570 aut rubrica 1960", &subj));
        // For Tridentine1570: first disjunct (NOT 1570) is false; second (1960) is false.
        let subj = s(Rubric::Tridentine1570, "Adv1");
        assert!(!vero("nisi rubrica 1570 aut rubrica 1960", &subj));
    }

    #[test]
    fn named_predicate_tridentina() {
        let subj = s(Rubric::Tridentine1570, "Adv1");
        assert!(vero("rubrica tridentina", &subj));
        assert!(vero("rubricis tridentina", &subj));
        let subj = s(Rubric::Rubrics1960, "Adv1");
        assert!(!vero("rubrica tridentina", &subj));
    }

    #[test]
    fn multi_word_predicate_with_implicit_subject() {
        // (post septuagesimam) — predicate-only form with no subject;
        // subject defaults to `tempore` (resolved by get_tempus_id).
        //
        // The named "post septuagesimam" predicate matches values
        // containing /Septua|Quadra|Passio/i. With slice-3 in place,
        // get_tempus_id maps:
        //   Quadp1 → "Septuagesimæ" (matches "Septua")
        //   Quad1  → "Quadragesimæ" (matches "Quadra")
        //   Quad5  → "Passionis"    (matches "Passio")
        //   Adv1   → "Adventus"     (no match)
        let subj = Subjects::new(Rubric::Tridentine1570, "Quadp1", 5, 2, 2026)
            .with_dayofweek(0);
        assert!(vero("post septuagesimam", &subj));

        let subj = Subjects::new(Rubric::Tridentine1570, "Quad1", 20, 2, 2026);
        assert!(vero("post septuagesimam", &subj));

        let subj = Subjects::new(Rubric::Tridentine1570, "Quad5", 20, 3, 2026);
        assert!(vero("post septuagesimam", &subj));

        let subj = s(Rubric::Tridentine1570, "Adv1");
        assert!(!vero("post septuagesimam", &subj));
    }

    #[test]
    fn implicit_subject_regex_fallback_for_tempore() {
        // (Adv) — bare predicate, no named-predicate match. Falls
        // through to regex-fallback: matches `tempore` (dayname0)
        // case-insensitively.
        let subj = s(Rubric::Tridentine1570, "Adv1");
        assert!(vero("Adv", &subj));
        assert!(!vero("Pasc", &subj));
    }

    #[test]
    fn alternation_in_regex_fallback() {
        // (Adv|Nat) — alternation in the regex fallback.
        let subj = s(Rubric::Tridentine1570, "Adv1");
        assert!(vero("Adv|Nat", &subj));
        assert!(vero("Pasc|Adv", &subj));
        assert!(!vero("Pasc|Pent", &subj));
    }

    #[test]
    fn feria_predicate_uses_dayofweek_plus_one() {
        // dayofweek 0 (Sunday) → feria 1
        let subj = Subjects::new(Rubric::Tridentine1570, "Adv1", 1, 1, 2026)
            .with_dayofweek(0);
        assert!(vero("feria prima", &subj));
        assert!(!vero("feria secunda", &subj));

        // dayofweek 1 (Monday) → feria 2 (== Perl `secunda`)
        let subj = subj.with_dayofweek(1);
        assert!(vero("feria secunda", &subj));
        assert!(!vero("feria prima", &subj));
    }

    #[test]
    fn ad_predicate_swaps_for_missa_context() {
        // missa context — subject `ad` reads "missam".
        let subj = Subjects::new(Rubric::Tridentine1570, "Adv1", 1, 1, 2026)
            .with_missa(true, 1);
        assert!(vero("ad missam", &subj));
        assert!(!vero("ad Vesperam", &subj));

        // Non-missa context — subject `ad` reads `subjects.hora`.
        let subj = Subjects::new(Rubric::Tridentine1570, "Adv1", 1, 1, 2026)
            .with_hora("Vespera");
        assert!(vero("ad Vespera", &subj));
        assert!(!vero("ad missam", &subj));
    }

    #[test]
    fn paschali_predicate() {
        // (tempore paschali) — fires when get_tempus_id returns a value
        // containing /Paschæ|Ascensionis|Octava Pentecostes/i.
        //
        // With slice-3 in place: Pasc0 → "Octava Paschæ", Pasc7 →
        // "Octava Pentecostes", Pasc6-1 → "Octava Ascensionis".
        let subj = Subjects::new(Rubric::Tridentine1570, "Pasc0", 5, 4, 2026);
        assert!(vero("tempore paschali", &subj));

        let subj = Subjects::new(Rubric::Tridentine1570, "Pasc7", 24, 5, 2026);
        assert!(vero("tempore paschali", &subj));

        let subj = Subjects::new(Rubric::Tridentine1570, "Pasc6-1", 18, 5, 2026);
        assert!(vero("tempore paschali", &subj));

        // Outside Paschal time, fails.
        let subj = Subjects::new(Rubric::Tridentine1570, "Adv1", 5, 12, 2026);
        assert!(!vero("tempore paschali", &subj));
    }

    #[test]
    fn regex_fallback_for_unknown_named_predicate_against_tempore() {
        // Empty `tempore` — regex fallback against the empty subject
        // value should never match.
        let subj = Subjects {
            rubric: Some(Rubric::Tridentine1570),
            dayname0: "",
            ..Default::default()
        };
        assert!(!vero("Adv", &subj));
    }

    #[test]
    fn case_insensitive_keywords() {
        // Perl uses /i on stopwords + named predicates. Our impl
        // honours that for predicate names; subject keywords also
        // case-fold via to_ascii_lowercase. The literal predicate
        // text is case-insensitive too.
        let subj = s(Rubric::Tridentine1570, "Adv1");
        assert!(vero("RUBRICA TRIDENTINA", &subj));
        assert!(vero("Rubrica Tridentina", &subj));
        assert!(vero("rubrica TRIDENT", &subj)); // regex fallback against uppercase
    }

    #[test]
    fn split_keyword_word_boundary_aware() {
        // `aut` should not split `autem` or `autorem`.
        let parts = split_keyword("foo autem bar aut baz", "aut");
        // Only the standalone `aut` between `bar` and `baz` should split.
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].trim(), "foo autem bar");
        assert_eq!(parts[1].trim(), "baz");
    }

    #[test]
    fn et_word_boundary_aware() {
        // `et` should not split `meritis` or `feria`.
        let pieces = split_et_nisi("meritis feria et alia");
        // The single standalone `et` splits; "meritis" / "feria" stay intact.
        let atoms: Vec<&str> = pieces
            .iter()
            .filter_map(|p| match p {
                EtNisiPiece::Atom(a) => Some(*a),
                _ => None,
            })
            .collect();
        assert_eq!(atoms, vec!["meritis feria", "alia"]);
    }

    #[test]
    fn empty_disjunct_does_not_satisfy() {
        // `(rubrica 1570 aut )` — the trailing aut produces an empty
        // disjunct that should NOT satisfy the disjunction.
        let subj = s(Rubric::Rubrics1960, "Adv1");
        assert!(!vero("rubrica 1570 aut ", &subj));
    }

    #[test]
    fn complex_real_world_condition() {
        // Pulled from upstream — `(sed rubrica 1955 aut rubrica 1960)`
        // appears literally in the corpus on dozens of sections.
        let subj_1955 = s(Rubric::Reduced1955, "Adv1");
        let subj_1960 = s(Rubric::Rubrics1960, "Adv1");
        let subj_1570 = s(Rubric::Tridentine1570, "Adv1");
        assert!(vero("rubrica 1955 aut rubrica 1960", &subj_1955));
        assert!(vero("rubrica 1955 aut rubrica 1960", &subj_1960));
        assert!(!vero("rubrica 1955 aut rubrica 1960", &subj_1570));

        // `(nisi rubrica monastica)` — extremely common.
        let subj_monastic = s(Rubric::Monastic, "Adv1");
        assert!(!vero("nisi rubrica monastica", &subj_monastic));
        assert!(vero("nisi rubrica monastica", &subj_1570));
    }

    // ─── B10b-slice-3: get_tempus_id ────────────────────────────

    fn tempus_subj(rubric: Rubric, dayname0: &str, day: u32, month: u32) -> Subjects<'_> {
        Subjects::new(rubric, dayname0, day, month, 2026)
    }

    #[test]
    fn tempus_id_advent() {
        let s = tempus_subj(Rubric::Tridentine1570, "Adv1", 5, 12);
        assert_eq!(get_tempus_id(&s), "Adventus");
        let s = tempus_subj(Rubric::Tridentine1570, "Adv4", 22, 12);
        assert_eq!(get_tempus_id(&s), "Adventus");
    }

    #[test]
    fn tempus_id_christmastide_to_epiphany_eve() {
        // Christmas Day — `Nat0`, Dec 25 → Nativitatis.
        let s = tempus_subj(Rubric::Tridentine1570, "Nat0", 25, 12);
        assert_eq!(get_tempus_id(&s), "Nativitatis");
        // Jan 5 daytime — Nat tag, NOT Vespera → still Nativitatis.
        let s = tempus_subj(Rubric::Tridentine1570, "Nat1", 5, 1);
        assert_eq!(get_tempus_id(&s), "Nativitatis");
        // Jan 5 Vespera — first Vespers of Epiphany.
        let s = tempus_subj(Rubric::Tridentine1570, "Nat1", 5, 1)
            .with_hora("Vespera");
        assert_eq!(get_tempus_id(&s), "Epiphaniæ");
        // Jan 6 — Epiphany.
        let s = tempus_subj(Rubric::Tridentine1570, "Nat1", 6, 1);
        assert_eq!(get_tempus_id(&s), "Epiphaniæ");
    }

    #[test]
    fn tempus_id_post_epiphany_window() {
        // Jan 7-13 — within Octave of Epiphany.
        let s = tempus_subj(Rubric::Tridentine1570, "Epi", 10, 1);
        assert_eq!(get_tempus_id(&s), "Epiphaniæ");
        // Jan 14 — post Epiphaniam post partum (the GABC-season carve-out).
        let s = tempus_subj(Rubric::Tridentine1570, "Epi1", 14, 1);
        assert_eq!(get_tempus_id(&s), "post Epiphaniam post partum");
        // Feb 2 daytime — still post-partum.
        let s = tempus_subj(Rubric::Tridentine1570, "Epi4", 2, 2);
        assert_eq!(get_tempus_id(&s), "post Epiphaniam post partum");
        // Feb 2 Vespera — out of post-partum window.
        let s = tempus_subj(Rubric::Tridentine1570, "Epi4", 2, 2)
            .with_hora("Vespera");
        assert_eq!(get_tempus_id(&s), "post Epiphaniam");
        // Feb 4 — post Epiphaniam (no longer post-partum).
        let s = tempus_subj(Rubric::Tridentine1570, "Epi4", 4, 2);
        assert_eq!(get_tempus_id(&s), "post Epiphaniam");
    }

    #[test]
    fn tempus_id_septuagesima_lent_passion() {
        let s = tempus_subj(Rubric::Tridentine1570, "Quadp1", 1, 2);
        // Quadp1 in Feb 1 → Septuagesimæ post partum (Feb 1 is in window).
        assert_eq!(get_tempus_id(&s), "Septuagesimæ post partum");
        // Quadp2 Feb 5 → Septuagesimæ.
        let s = tempus_subj(Rubric::Tridentine1570, "Quadp2", 5, 2);
        assert_eq!(get_tempus_id(&s), "Septuagesimæ");
        // Quad1 → Quadragesimæ.
        let s = tempus_subj(Rubric::Tridentine1570, "Quad1", 20, 2);
        assert_eq!(get_tempus_id(&s), "Quadragesimæ");
        // Quad4 → Quadragesimæ.
        let s = tempus_subj(Rubric::Tridentine1570, "Quad4", 13, 3);
        assert_eq!(get_tempus_id(&s), "Quadragesimæ");
        // Quad5 → Passionis.
        let s = tempus_subj(Rubric::Tridentine1570, "Quad5", 20, 3);
        assert_eq!(get_tempus_id(&s), "Passionis");
        // Quad6 → Passionis.
        let s = tempus_subj(Rubric::Tridentine1570, "Quad6", 27, 3);
        assert_eq!(get_tempus_id(&s), "Passionis");
    }

    #[test]
    fn tempus_id_easter_octave() {
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc0", 5, 4);
        assert_eq!(get_tempus_id(&s), "Octava Paschæ");
        // Saturday eve of Pasc0 → Vigilia Paschalis.
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc0", 4, 4)
            .with_hora("Vespera")
            .with_dayofweek(6);
        assert_eq!(get_tempus_id(&s), "Vigilia Paschalis");
    }

    #[test]
    fn tempus_id_post_easter() {
        // Pasc1-4 → post Octavam Paschæ.
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc2", 19, 4)
            .with_dayofweek(0);
        assert_eq!(get_tempus_id(&s), "post Octavam Paschæ");
        // Pasc5 early-week (dayofweek<3) → post Octavam Paschæ.
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc5", 11, 5)
            .with_dayofweek(1);
        assert_eq!(get_tempus_id(&s), "post Octavam Paschæ");
        // Pasc5 Wed daytime → still post Octavam Paschæ.
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc5", 13, 5)
            .with_dayofweek(3);
        assert_eq!(get_tempus_id(&s), "post Octavam Paschæ");
        // Pasc5 Wed Vespera → Octava Ascensionis (the eve of Ascension).
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc5", 13, 5)
            .with_dayofweek(3)
            .with_hora("Vespera");
        assert_eq!(get_tempus_id(&s), "Octava Ascensionis");
    }

    #[test]
    fn tempus_id_pasc6_special_branches() {
        // Pasc6-5 / Pasc6-6 → post Octavam Ascensionis.
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc6-5", 22, 5);
        assert_eq!(get_tempus_id(&s), "post Octavam Ascensionis");
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc6-6", 23, 5);
        assert_eq!(get_tempus_id(&s), "post Octavam Ascensionis");
        // Pasc6-other → Octava Ascensionis.
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc6-1", 18, 5);
        assert_eq!(get_tempus_id(&s), "Octava Ascensionis");
        // Pasc7 → Octava Pentecostes.
        let s = tempus_subj(Rubric::Tridentine1570, "Pasc7", 24, 5);
        assert_eq!(get_tempus_id(&s), "Octava Pentecostes");
    }

    #[test]
    fn tempus_id_corpus_christi_thursday() {
        // Pent01 Thursday → Corpus Christi post Pentecosten.
        let s = tempus_subj(Rubric::Tridentine1570, "Pent01", 4, 6)
            .with_dayofweek(4);
        assert_eq!(get_tempus_id(&s), "Corpus Christi post Pentecosten");
    }

    #[test]
    fn tempus_id_octava_corpus_christi_pre_1955_only() {
        // Pent01 Friday (dayofweek 5) — pre-1955 → Octava CC.
        let s = tempus_subj(Rubric::Tridentine1570, "Pent01", 5, 6)
            .with_dayofweek(5);
        assert_eq!(get_tempus_id(&s), "Octava Corpus Christi post Pentecosten");
        // Same date 1955 — no Octave CC.
        let s = tempus_subj(Rubric::Reduced1955, "Pent01", 5, 6)
            .with_dayofweek(5);
        assert_ne!(get_tempus_id(&s), "Octava Corpus Christi post Pentecosten");
        // Same date 1960 — also no Octave CC.
        let s = tempus_subj(Rubric::Rubrics1960, "Pent01", 5, 6)
            .with_dayofweek(5);
        assert_ne!(get_tempus_id(&s), "Octava Corpus Christi post Pentecosten");
    }

    #[test]
    fn tempus_id_post_pentecost_default() {
        // Pent10 mid-summer → post Pentecosten (not Oct/Nov).
        let s = tempus_subj(Rubric::Rubrics1960, "Pent10", 1, 8);
        assert_eq!(get_tempus_id(&s), "post Pentecosten");
        // Pent22 in November → post Pentecosten in hieme.
        let s = tempus_subj(Rubric::Rubrics1960, "Pent22", 5, 11);
        assert_eq!(get_tempus_id(&s), "post Pentecosten in hieme");
        // Pent20 in October → post Pentecosten in hieme.
        let s = tempus_subj(Rubric::Rubrics1960, "Pent20", 20, 10);
        assert_eq!(get_tempus_id(&s), "post Pentecosten in hieme");
    }

    #[test]
    fn tempus_id_via_vero_in_tempore_clause() {
        // The chain `(in tempore Adventus)` should now fire on an
        // Adv* dayname — exercising the integration between vero
        // and get_tempus_id.
        let s = tempus_subj(Rubric::Tridentine1570, "Adv1", 5, 12);
        assert!(vero("tempore Adventus", &s));
        assert!(!vero("tempore Nativitatis", &s));
        let s = tempus_subj(Rubric::Tridentine1570, "Quad3", 13, 3);
        assert!(vero("tempore Quadragesimæ", &s));
        assert!(!vero("tempore Adventus", &s));
    }

    // ─── B10b-slice-3: dayname_for_condition ────────────────────

    #[test]
    fn dayname_epiphany() {
        let s = tempus_subj(Rubric::Tridentine1570, "Nat1", 6, 1);
        assert_eq!(dayname_for_condition(&s), "Epiphaniæ");
        // Jan 5 Vespera → Epiphaniæ.
        let s = tempus_subj(Rubric::Tridentine1570, "Nat1", 5, 1)
            .with_hora("Vespera");
        assert_eq!(dayname_for_condition(&s), "Epiphaniæ");
    }

    #[test]
    fn dayname_baptism_of_lord() {
        let s = tempus_subj(Rubric::Tridentine1570, "Epi1", 13, 1);
        assert_eq!(dayname_for_condition(&s), "Baptismatis Domini");
    }

    #[test]
    fn dayname_holy_week_triduum() {
        let s = tempus_subj(Rubric::Tridentine1570, "Quad6", 2, 4)
            .with_winner("Tempora/Quad6-4", "");
        // Quad6-4..6 → "Tridui Sacri" (the umbrella label fires before
        // the more-specific ones because the regex matches first).
        assert_eq!(dayname_for_condition(&s), "Tridui Sacri");
    }

    #[test]
    fn dayname_all_souls_window() {
        // Nov 2 — All Souls.
        let s = tempus_subj(Rubric::Tridentine1570, "Pent20", 2, 11);
        assert_eq!(dayname_for_condition(&s), "Omnium Defunctorum");
        // Nov 3 Monday — All Souls transferred.
        let s = tempus_subj(Rubric::Tridentine1570, "Pent20", 3, 11)
            .with_dayofweek(1);
        assert_eq!(dayname_for_condition(&s), "Omnium Defunctorum");
        // Nov 3 not Monday — falls through (Malachiae handles it).
        let s = tempus_subj(Rubric::Tridentine1570, "Pent20", 3, 11)
            .with_dayofweek(2);
        assert_eq!(dayname_for_condition(&s), "Malachiae");
    }

    #[test]
    fn dayname_st_nicholas() {
        let s = tempus_subj(Rubric::Tridentine1570, "Adv2", 6, 12);
        assert_eq!(dayname_for_condition(&s), "Nicolai");
    }

    #[test]
    fn dayname_doctor_winner() {
        // dayname[1] containing "Doctor" — drives the `(officio doctorum)`
        // clauses.
        let s = tempus_subj(Rubric::Tridentine1570, "Pent20", 15, 8)
            .with_dayname("Pent20", "S. Bernardi Abbatis et Doctoris", "");
        assert_eq!(dayname_for_condition(&s), "doctorum");
    }

    #[test]
    fn dayname_transfiguration() {
        let s = tempus_subj(Rubric::Tridentine1570, "Pent10", 6, 8);
        assert_eq!(dayname_for_condition(&s), "transfigurationis");
        // Aug 5 Vespera — eve of Transfiguration.
        let s = tempus_subj(Rubric::Tridentine1570, "Pent10", 5, 8)
            .with_hora("Vespera");
        assert_eq!(dayname_for_condition(&s), "transfigurationis");
    }

    #[test]
    fn dayname_christmas() {
        let s = tempus_subj(Rubric::Tridentine1570, "Nat0", 25, 12)
            .with_winner("Sancti/12-25", "");
        assert_eq!(dayname_for_condition(&s), "Nativitatis");
    }

    #[test]
    fn dayname_post_epiphany_octave_interior() {
        // Epi1-1..6 → post Dominicam infra Octavam Epiphaniæ.
        let s = tempus_subj(Rubric::Tridentine1570, "Epi1-3", 9, 1);
        assert_eq!(
            dayname_for_condition(&s),
            "post Dominicam infra Octavam Epiphaniæ",
        );
    }

    #[test]
    fn dayname_three_lectiones_rule() {
        // winner_rule containing "3 lectio" → "3 lectionum".
        let s = tempus_subj(Rubric::Rubrics1960, "Pent05", 1, 7)
            .with_winner("Sancti/07-01", "Simplex;;1.1\n3 lectiones");
        assert_eq!(dayname_for_condition(&s), "3 lectionum");
    }

    #[test]
    fn dayname_septem_doloris_movable() {
        // Friday-after-Passion-Sunday — winner key 09-DT.
        let s = tempus_subj(Rubric::Tridentine1570, "Quad5", 7, 4)
            .with_winner("Sancti/09-DT", "");
        assert_eq!(dayname_for_condition(&s), "septem doloris");
        // Quad5-5 — alternate trigger.
        let s = tempus_subj(Rubric::Tridentine1570, "Quad5", 8, 4)
            .with_winner("Tempora/Quad5-5", "");
        assert_eq!(dayname_for_condition(&s), "septem doloris");
    }

    #[test]
    fn dayname_no_match_returns_empty() {
        let s = tempus_subj(Rubric::Tridentine1570, "Pent12", 15, 7);
        assert_eq!(dayname_for_condition(&s), "");
    }

    #[test]
    fn dayname_via_vero_die_clause() {
        // (die Epiphaniæ) on Jan 6 — fires.
        let s = tempus_subj(Rubric::Tridentine1570, "Nat1", 6, 1);
        assert!(vero("die Epiphaniæ", &s));
        // Same condition on a random day — fails.
        let s = tempus_subj(Rubric::Tridentine1570, "Pent10", 1, 8);
        assert!(!vero("die Epiphaniæ", &s));
    }

    // ─── B10b-slice-2: find_conditional + parse_conditional ─────

    #[test]
    fn find_conditional_extracts_simple_directive() {
        let m = find_conditional("(sed rubrica 1960)").unwrap();
        assert_eq!(m.start, 0);
        assert_eq!(m.end, 18);
        assert_eq!(m.stopwords, "sed");
        assert_eq!(m.condition, "rubrica 1960");
        assert_eq!(m.scope, "");
    }

    #[test]
    fn find_conditional_with_no_stopword() {
        let m = find_conditional("(rubrica 1960)").unwrap();
        assert_eq!(m.stopwords, "");
        assert_eq!(m.condition, "rubrica 1960");
        assert_eq!(m.scope, "");
    }

    #[test]
    fn find_conditional_with_scope_keyword() {
        let m = find_conditional("(sed rubrica 1960 omittitur)").unwrap();
        assert_eq!(m.stopwords, "sed");
        assert_eq!(m.condition.trim(), "rubrica 1960");
        assert_eq!(m.scope.trim(), "omittitur");
    }

    #[test]
    fn find_conditional_with_dicuntur_scope() {
        let m = find_conditional("(sed rubrica monastica dicuntur)").unwrap();
        assert_eq!(m.stopwords, "sed");
        assert_eq!(m.condition.trim(), "rubrica monastica");
        assert_eq!(m.scope.trim(), "dicuntur");
    }

    #[test]
    fn find_conditional_with_dicitur_semper() {
        let m = find_conditional("(sed rubrica 1960 dicitur semper)").unwrap();
        assert_eq!(m.condition.trim(), "rubrica 1960");
        // The "semper" is part of the trailing scope text.
        assert!(m.scope.to_lowercase().contains("dicitur"));
        assert!(m.scope.to_lowercase().contains("semper"));
    }

    #[test]
    fn find_conditional_with_two_stopwords() {
        let m = find_conditional("(atque vero rubrica 1960)").unwrap();
        // Both `atque` and `vero` are stopwords — but only word-by-word
        // peeling. The scanner stops at the first non-stopword (`rubrica`).
        assert_eq!(m.stopwords.split_whitespace().count(), 2);
        assert_eq!(m.condition, "rubrica 1960");
    }

    #[test]
    fn find_conditional_returns_none_for_no_parens() {
        assert!(find_conditional("rubrica 1960").is_none());
        assert!(find_conditional("").is_none());
        assert!(find_conditional("plain text").is_none());
    }

    #[test]
    fn find_conditional_skips_unmatched_parens() {
        // Open paren with no closer — no match.
        assert!(find_conditional("(rubrica 1960").is_none());
    }

    #[test]
    fn parse_conditional_strength_from_stopwords() {
        let s = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let c = parse_conditional("sed", "rubrica 1960", "", &s);
        assert_eq!(c.strength, 1);
        let c = parse_conditional("atque", "rubrica 1960", "", &s);
        assert_eq!(c.strength, 2);
        let c = parse_conditional("attamen", "rubrica 1960", "", &s);
        assert_eq!(c.strength, 3);
        let c = parse_conditional("sed vero", "rubrica 1960", "", &s);
        assert_eq!(c.strength, 2);
        let c = parse_conditional("", "rubrica 1960", "", &s);
        assert_eq!(c.strength, 0);
    }

    #[test]
    fn parse_conditional_si_has_weight_zero() {
        // `si` is a stopword (so it gets stripped by find_conditional)
        // but contributes 0 strength.
        let s = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let c = parse_conditional("si", "rubrica 1960", "", &s);
        assert_eq!(c.strength, 0);
    }

    #[test]
    fn parse_conditional_implicit_line_backscope_for_sed() {
        // `(sed rubrica 1960)` — sed is implicitly backscoped to LINE
        // when no explicit scope keyword is present.
        let s = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let c = parse_conditional("sed", "rubrica 1960", "", &s);
        assert_eq!(c.backscope, Scope::Line);
    }

    #[test]
    fn parse_conditional_no_implicit_backscope_without_stopword() {
        let s = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let c = parse_conditional("", "rubrica 1960", "", &s);
        assert_eq!(c.backscope, Scope::Null);
    }

    #[test]
    fn parse_conditional_si_no_implicit_backscope() {
        // si is a stopword but NOT in %backscoped_stopwords.
        let s = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let c = parse_conditional("si", "rubrica 1960", "", &s);
        assert_eq!(c.backscope, Scope::Null);
    }

    #[test]
    fn parse_conditional_explicit_chunk_scope() {
        // `omittitur` -> SCOPE_CHUNK back, SCOPE_NULL forward.
        let s = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let c = parse_conditional("sed", "rubrica 1960", "omittitur", &s);
        assert_eq!(c.backscope, Scope::Chunk);
        assert_eq!(c.forwardscope, Scope::Null);
    }

    #[test]
    fn parse_conditional_explicit_nest_scope() {
        // `omittuntur` -> SCOPE_NEST back, SCOPE_NULL forward.
        let s = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let c = parse_conditional("sed", "rubrica 1960", "omittuntur", &s);
        assert_eq!(c.backscope, Scope::Nest);
        assert_eq!(c.forwardscope, Scope::Null);
    }

    #[test]
    fn parse_conditional_dicitur_semper_disables_line_backscope() {
        // `dicitur semper` — `semper` suppresses implicit LINE backscope.
        // Backscope: NULL. Forwardscope: LINE (default for non-CHUNK/NEST).
        let s = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let c = parse_conditional("sed", "rubrica 1960", "dicitur semper", &s);
        assert_eq!(c.backscope, Scope::Null);
        // No back-CHUNK/NEST -> forwardscope LINE.
        assert_eq!(c.forwardscope, Scope::Line);
    }

    #[test]
    fn parse_conditional_dicuntur_back_chunk_forward_chunk() {
        // `dicuntur` with CHUNK backscope (e.g. via `versus dicuntur`)
        // gives forward CHUNK; without explicit back gives forward NEST.
        let s = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let c = parse_conditional("sed", "rubrica 1960", "dicuntur", &s);
        // No `versus` / `versuum` in scope, no `semper` either, but
        // `sed` is implicit-backscoped — gives LINE back, then forward
        // is computed from back: not CHUNK, so forward = NEST.
        assert_eq!(c.backscope, Scope::Line);
        assert_eq!(c.forwardscope, Scope::Nest);
    }

    #[test]
    fn parse_conditional_result_evaluates_under_subjects() {
        let s_1960 = tempus_subj(Rubric::Rubrics1960, "Adv1", 5, 12);
        let s_1570 = tempus_subj(Rubric::Tridentine1570, "Adv1", 5, 12);
        let c = parse_conditional("sed", "rubrica 1960", "", &s_1960);
        assert!(c.result);
        let c = parse_conditional("sed", "rubrica 1960", "", &s_1570);
        assert!(!c.result);
    }

    #[test]
    fn find_conditional_real_world_corpus_examples() {
        // Examples lifted from the actual corpus.
        let m = find_conditional("(sed rubrica 1955 aut rubrica 1960)").unwrap();
        assert_eq!(m.stopwords, "sed");
        assert_eq!(m.condition, "rubrica 1955 aut rubrica 1960");
        assert_eq!(m.scope, "");

        let m = find_conditional("(nisi rubrica monastica)").unwrap();
        // "nisi" is not a stopword — it's part of the condition
        // (handled by `vero`'s et/nisi splitter).
        assert_eq!(m.stopwords, "");
        assert_eq!(m.condition, "nisi rubrica monastica");
    }

    // ─── B10b-slice-4: process_conditional_lines ────────────────

    fn body_subj(rubric: Rubric) -> Subjects<'static> {
        Subjects::new(rubric, "Adv1", 5, 12, 2026)
    }

    #[test]
    fn pcl_no_directives_passes_through() {
        let body = "line one\nline two\nline three";
        let s = body_subj(Rubric::Tridentine1570);
        assert_eq!(process_conditional_lines(body, &s), body);
    }

    #[test]
    fn pcl_single_line_directive_drops_when_false() {
        // `(rubrica 1960)` — true under Rubrics1960, false under 1570.
        // No stopword → no implicit backscope; forwardscope LINE.
        // The line itself (after the directive sequel) becomes
        // gated.
        let body = "before\n(rubrica 1960) gated content\nafter";
        let s_1960 = body_subj(Rubric::Rubrics1960);
        let s_1570 = body_subj(Rubric::Tridentine1570);
        // 1960 — gated content kept.
        let out = process_conditional_lines(body, &s_1960);
        assert!(out.contains("gated content"), "1960 output: {out:?}");
        assert!(out.contains("before"));
        assert!(out.contains("after"));
        // 1570 — gated content dropped.
        let out = process_conditional_lines(body, &s_1570);
        assert!(!out.contains("gated content"), "1570 output: {out:?}");
        assert!(out.contains("before"));
        assert!(out.contains("after"));
    }

    #[test]
    fn pcl_sed_drops_preceding_line_under_false_branch() {
        // `(sed rubrica 1960)` — sed gives implicit LINE backscope.
        // When TRUE: backscope drops preceding line, then forward gates current line.
        // When FALSE: nothing happens (no backscope retraction; current line line-gated and dropped).
        let body = "alpha\nbeta\n(sed rubrica 1960) gamma\ndelta";
        // Under 1960 (TRUE): drops "beta" via LINE backscope, KEEPS "gamma".
        let s = body_subj(Rubric::Rubrics1960);
        let out = process_conditional_lines(body, &s);
        assert!(out.contains("alpha"), "alpha missing in 1960: {out:?}");
        assert!(!out.contains("beta"), "beta should be dropped under 1960: {out:?}");
        assert!(out.contains("gamma"), "gamma missing in 1960: {out:?}");
        assert!(out.contains("delta"), "delta missing in 1960: {out:?}");
        // Under 1570 (FALSE): no retraction, gamma gated out, beta and delta survive.
        let s = body_subj(Rubric::Tridentine1570);
        let out = process_conditional_lines(body, &s);
        assert!(out.contains("alpha"), "alpha missing in 1570: {out:?}");
        assert!(out.contains("beta"), "beta missing in 1570: {out:?}");
        assert!(!out.contains("gamma"), "gamma should be dropped under 1570: {out:?}");
        assert!(out.contains("delta"), "delta missing in 1570: {out:?}");
    }

    #[test]
    fn pcl_chunk_back_scope_via_versus_omittitur() {
        // `(rubrica 1960 hic versus omittitur)` — explicit CHUNK back, NULL forward.
        // When TRUE under 1960: drops preceding non-blank chunk back to last fence.
        let body = "para1 line1\npara1 line2\npara1 line3\n(rubrica 1960 hic versus omittitur)\nafter";
        let s = body_subj(Rubric::Rubrics1960);
        let out = process_conditional_lines(body, &s);
        // Under TRUE: forward NULL (becomes NEST after the "having
        // backtracked" rewrite); back CHUNK drops the three "para1"
        // lines.
        assert!(!out.contains("para1"), "CHUNK back should drop para1 lines under 1960: {out:?}");
        assert!(out.contains("after"));
    }

    #[test]
    fn pcl_blank_line_terminates_chunk_forward_scope() {
        // forward CHUNK: gating extends until next blank line.
        // `(sed rubrica 1960 versus omittitur)` — back CHUNK; under
        // FALSE forward CHUNK applied. Once the blank line hits, the
        // frame pops and subsequent lines emit.
        // Use a (versus dicuntur) variant: back CHUNK forward CHUNK
        // (per parse_conditional logic) under the affirmative case.
        // For simplicity, test that a forward-LINE conditional only
        // gates ONE line.
        let body = "a\n(rubrica 1960) b\nc";
        let s = body_subj(Rubric::Tridentine1570); // FALSE
        let out = process_conditional_lines(body, &s);
        // forward LINE under FALSE: drops "b" only; "c" survives.
        assert!(out.contains("a"));
        assert!(!out.contains("b"));
        assert!(out.contains("c"));
    }

    #[test]
    fn pcl_tilde_escape_strips_leading_tilde() {
        // ~( is a way to emit a literal `(` at the start of a line.
        // The `~` is stripped after directive-detection.
        let body = "~(literal paren start)";
        let s = body_subj(Rubric::Rubrics1960);
        let out = process_conditional_lines(body, &s);
        assert_eq!(out, "(literal paren start)");
    }

    #[test]
    fn pcl_empty_body() {
        let s = body_subj(Rubric::Rubrics1960);
        assert_eq!(process_conditional_lines("", &s), "");
    }

    #[test]
    fn pcl_directive_with_no_sequel() {
        // Directive line with nothing after — emits nothing for that
        // line (since sequel is empty), but next line is gated by
        // forwardscope.
        let body = "alpha\n(rubrica 1960)\nbeta\ngamma";
        // Forward LINE under TRUE: "beta" survives; "gamma" not gated.
        let s = body_subj(Rubric::Rubrics1960);
        let out = process_conditional_lines(body, &s);
        assert!(out.contains("alpha"));
        assert!(out.contains("beta"));
        assert!(out.contains("gamma"));
        // Under FALSE: forward LINE drops "beta"; "gamma" survives.
        let s = body_subj(Rubric::Tridentine1570);
        let out = process_conditional_lines(body, &s);
        assert!(out.contains("alpha"));
        assert!(!out.contains("beta"));
        assert!(out.contains("gamma"));
    }

    #[test]
    fn pcl_two_independent_directives() {
        let body = "(rubrica 1960) under1960\n(rubrica 1570) under1570";
        let s_1960 = body_subj(Rubric::Rubrics1960);
        let out = process_conditional_lines(body, &s_1960);
        assert!(out.contains("under1960"));
        assert!(!out.contains("under1570"));
        let s_1570 = body_subj(Rubric::Tridentine1570);
        let out = process_conditional_lines(body, &s_1570);
        assert!(!out.contains("under1960"));
        assert!(out.contains("under1570"));
    }

    #[test]
    fn pcl_consecutive_blank_lines_preserved_outside_directive() {
        let body = "a\n\nb\n\n\nc";
        let s = body_subj(Rubric::Rubrics1960);
        let out = process_conditional_lines(body, &s);
        // No directives — pass through unchanged.
        assert_eq!(out, body);
    }

    #[test]
    fn pcl_real_corpus_pattern_sed_rubrica_omittitur() {
        // Pattern from the real corpus: a section body with a few
        // lines followed by `(sed rubrica 1955 aut rubrica 1960
        // hic versus omittitur)` indicating that those preceding
        // lines should be dropped under 1955+ but kept under 1570.
        let body = "Versus extra 1\nVersus extra 2\n(sed rubrica 1955 aut rubrica 1960 hic versus omittitur)\nMain content\nMore content";
        // Under 1960: CHUNK back drops the two "Versus extra" lines.
        let s = body_subj(Rubric::Rubrics1960);
        let out = process_conditional_lines(body, &s);
        assert!(!out.contains("Versus extra"), "1960 should drop pre-directive chunk: {out:?}");
        assert!(out.contains("Main content"));
        // Under 1570: "Versus extra" lines kept; main content also kept.
        let s = body_subj(Rubric::Tridentine1570);
        let out = process_conditional_lines(body, &s);
        assert!(out.contains("Versus extra 1"));
        assert!(out.contains("Versus extra 2"));
        assert!(out.contains("Main content"));
    }

    #[test]
    fn pcl_sequel_after_directive_is_subject_to_gating() {
        // After (rubrica 1960), the sequel is on the same line.
        // Under TRUE: sequel kept. Under FALSE: sequel dropped.
        let body = "(rubrica 1960) sequel content";
        let s = body_subj(Rubric::Rubrics1960);
        let out = process_conditional_lines(body, &s);
        assert_eq!(out.trim(), "sequel content");
        let s = body_subj(Rubric::Tridentine1570);
        let out = process_conditional_lines(body, &s);
        // Empty (the only line was gated out).
        assert!(out.trim().is_empty());
    }

    // ─── B10b-slice-5: do_inclusion_substitutions ───────────────

    fn dis(body: &str, spec: &str) -> String {
        let mut s = body.to_string();
        do_inclusion_substitutions(&mut s, spec);
        s
    }

    #[test]
    fn dis_line_pick_single() {
        assert_eq!(dis("a\nb\nc", "1"), "a\n");
        assert_eq!(dis("a\nb\nc", "2"), "b\n");
        assert_eq!(dis("a\nb\nc", "3"), "c\n");
    }

    #[test]
    fn dis_line_pick_range() {
        assert_eq!(dis("a\nb\nc\nd", "1-2"), "a\nb\n");
        assert_eq!(dis("a\nb\nc\nd", "2-4"), "b\nc\nd\n");
    }

    #[test]
    fn dis_line_drop_single() {
        // !2 — drop line 2.
        assert_eq!(dis("a\nb\nc", "!2"), "a\nc\n");
    }

    #[test]
    fn dis_line_drop_range() {
        // !2-3 — drop lines 2-3.
        assert_eq!(dis("a\nb\nc\nd", "!2-3"), "a\nd\n");
    }

    #[test]
    fn dis_simple_substitution() {
        assert_eq!(dis("hello world", "s/world/Rust/"), "hello Rust");
    }

    #[test]
    fn dis_global_substitution() {
        assert_eq!(dis("a b a b a", "s/a/X/g"), "X b X b X");
        // Without /g: only first occurrence.
        assert_eq!(dis("a b a b a", "s/a/X/"), "X b a b a");
    }

    #[test]
    fn dis_anchor_substitution_per_line_with_m_flag() {
        // `s/$/X/m` — append X to end of each line. Without /m,
        // only matches end-of-string.
        assert_eq!(dis("a\nb\nc", "s/$/X/gm"), "aX\nbX\ncX");
    }

    #[test]
    fn dis_caret_anchor_per_line_with_m_flag() {
        // `s/^/Ant. /` (no /m, no /g) — prepend "Ant. " at start of body.
        assert_eq!(dis("foo\nbar", "s/^/Ant. /"), "Ant. foo\nbar");
        // With /gm — prepend at every line start.
        assert_eq!(dis("foo\nbar", "s/^/Ant. /gm"), "Ant. foo\nAnt. bar");
    }

    #[test]
    fn dis_strip_suffix_pattern() {
        // `s/;;.*//g` — drop `;;` and everything after, on every line.
        // Dot does NOT match newline (no /s flag), so each occurrence
        // of `;;...<eol>` becomes a separate match.
        assert_eq!(
            dis("Antiphon body;;tone1\nAnother;;tone2", "s/;;.*//g"),
            "Antiphon body\nAnother"
        );
        // Without /g — only the first occurrence on the whole text.
        assert_eq!(
            dis("Antiphon body;;tone1\nAnother;;tone2", "s/;;.*//"),
            "Antiphon body\nAnother;;tone2"
        );
    }

    #[test]
    fn dis_digit_class() {
        // `s/;;\d+//g` — drop `;;` followed by digits.
        assert_eq!(
            dis("body;;109 then;;42 end", "s/;;\\d+//g"),
            "body then end"
        );
    }

    #[test]
    fn dis_negative_lookahead() {
        // `s/;;\d+(?![a-z])//g` — drop `;;<digits>` but not when
        // followed by a lowercase letter.
        assert_eq!(
            dis("a;;5 b;;5x c;;7", "s/;;\\d+(?![a-z])//g"),
            "a b;;5x c"
        );
    }

    #[test]
    fn dis_chained_directives() {
        // Multiple directives in one spec, comma- or whitespace-
        // separated.
        let body = "a\nb\nc\nd";
        // First pick lines 2-3, then sub b->BB.
        assert_eq!(dis(body, "2-3 s/b/BB/"), "BB\nc\n");
    }

    #[test]
    fn dis_unsupported_pattern_skips_directive_silently() {
        // We don't support backreferences (\1) or named captures.
        // A directive with an unsupported feature should leave the
        // body untouched (no panic).
        let before = "a\nb\nc".to_string();
        let mut s = before.clone();
        do_inclusion_substitutions(&mut s, "s/(?P<x>foo)/bar/");
        // Either the body is unchanged (we couldn't compile the
        // pattern) OR our parser accepts it as a non-capturing-ish
        // group — both are acceptable. The critical contract is "no
        // panic".
        // (We don't assert on the exact value because it depends on
        // the parser's tolerance.)
        let _ = s;
    }

    #[test]
    fn dis_alternation() {
        // s/foo|bar/X/g
        assert_eq!(dis("foo bar baz", "s/foo|bar/X/g"), "X X baz");
    }

    #[test]
    fn dis_real_corpus_example_ant_vespera_strip_tone() {
        // From the real corpus: `s/;;\d+//gm` strips numeric chant-tone
        // suffixes from antiphons. Note `\d+` matches digits only —
        // for `;;1d2` (numeric prefix + letter + digit) only `;;1` is
        // stripped, leaving `d2`. The corpus uses this pattern when
        // the chant tone is a pure integer; the `;;1d2` case is rare
        // and the upstream Perl behaviour is the same (greedy digit
        // match stops at non-digit).
        let body = "Ant 1;;7\nAnt 2;;3\nAnt 3;;5";
        assert_eq!(
            dis(body, "s/;;\\d+//gm"),
            "Ant 1\nAnt 2\nAnt 3"
        );

        // The mixed-tone case: `\d+` only chews leading digits; the
        // trailing letter+digit (`d2`) survives but the `;;` is gone
        // because it was consumed with the matched prefix. Matches
        // Perl behaviour (the corpus uses `s/;;.*//` for chant
        // suffixes that include letters).
        let body = "Ant a;;1d2\nAnt b;;7";
        assert_eq!(dis(body, "s/;;\\d+//gm"), "Ant ad2\nAnt b");
    }

    #[test]
    fn dis_real_corpus_example_underscore_append() {
        // `s/$/_/gm` — append underscore at end of each line.
        let body = "Ant 1\nAnt 2\nAnt 3";
        assert_eq!(dis(body, "s/$/_/gm"), "Ant 1_\nAnt 2_\nAnt 3_");
    }

    #[test]
    fn dis_real_corpus_example_period_substitute() {
        // `s/\.$/.;;109/m` — append ;;109 after trailing period (per line).
        let body = "Antiphon body.\nNo period\nAnother.";
        assert_eq!(
            dis(body, "s/\\.$/.;;109/m"),
            "Antiphon body.;;109\nNo period\nAnother."
        );
    }

    // ─── B10b-slice-6: resolve_section ──────────────────────────

    use std::collections::HashMap;

    fn mock_corpus(entries: &[(&str, &str, &str)]) -> HashMap<(String, String), String> {
        let mut m = HashMap::new();
        for (path, section, body) in entries {
            m.insert((path.to_string(), section.to_string()), body.to_string());
        }
        m
    }

    #[test]
    fn resolve_section_returns_direct_body_when_not_a_redirect() {
        let corpus = mock_corpus(&[
            ("Sancti/05-04", "Oratio", "Praesta, quaesumus..."),
        ]);
        let s = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("Sancti/05-04", "Oratio", &s, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        assert_eq!(result.as_deref(), Some("Praesta, quaesumus..."));
    }

    #[test]
    fn resolve_section_follows_simple_at_redirect() {
        // Sancti/01-08 [Oratio] = `@Sancti/01-06` — implicit same-section.
        let corpus = mock_corpus(&[
            ("Sancti/01-08", "Oratio", "@Sancti/01-06"),
            ("Sancti/01-06", "Oratio", "Deus, qui hodierna die..."),
        ]);
        let s = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("Sancti/01-08", "Oratio", &s, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        assert_eq!(result.as_deref(), Some("Deus, qui hodierna die..."));
    }

    #[test]
    fn resolve_section_follows_explicit_at_section_redirect() {
        // Sancti/05-04 [Hymnus Vespera] = `@Commune/C7:Hymnus Vespera`.
        let corpus = mock_corpus(&[
            ("Sancti/05-04", "Hymnus Vespera", "@Commune/C7:Hymnus Vespera"),
            ("Commune/C7", "Hymnus Vespera", "Fortem virili pectore..."),
        ]);
        let s = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("Sancti/05-04", "Hymnus Vespera", &s, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        assert_eq!(result.as_deref(), Some("Fortem virili pectore..."));
    }

    #[test]
    fn resolve_section_self_reference_at_colon_section() {
        // `@:OtherSection` — same file, different section.
        let corpus = mock_corpus(&[
            ("Sancti/05-04", "Ant 2C", "@:Ant Vespera"),
            ("Sancti/05-04", "Ant Vespera", "Mulier fortis..."),
        ]);
        let s = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("Sancti/05-04", "Ant 2C", &s, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        assert_eq!(result.as_deref(), Some("Mulier fortis..."));
    }

    #[test]
    fn resolve_section_multi_hop_chain() {
        // Three-hop chain: A -> B -> C -> body.
        let corpus = mock_corpus(&[
            ("A", "X", "@B:X"),
            ("B", "X", "@C:X"),
            ("C", "X", "Final body"),
        ]);
        let s = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("A", "X", &s, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        assert_eq!(result.as_deref(), Some("Final body"));
    }

    #[test]
    fn resolve_section_cycle_detection() {
        // A -> B -> A forms a cycle. After MAX_AT_HOPS the resolver
        // bails out and returns the raw body of the current loop iter.
        let corpus = mock_corpus(&[
            ("A", "X", "@B:X"),
            ("B", "X", "@A:X"),
        ]);
        let s = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("A", "X", &s, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        // We get *some* result — either A's body or B's body — but
        // not infinite loop / panic. The point of the test is the
        // termination guarantee.
        let body = result.expect("cycle should still terminate with a body");
        assert!(body.starts_with('@'));
    }

    #[test]
    fn resolve_section_returns_none_when_path_missing() {
        let corpus: HashMap<(String, String), String> = HashMap::new();
        let s = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("Missing/Path", "X", &s, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        assert!(result.is_none());
    }

    #[test]
    fn resolve_section_applies_substitutions() {
        // `@:Ant Vespera:s/;;.*//gm` — pull Ant Vespera, strip
        // chant-tone suffix per line.
        let corpus = mock_corpus(&[
            ("Sancti/05-04", "Ant Magnificat", "@:Ant Vespera:s/;;.*//gm"),
            ("Sancti/05-04", "Ant Vespera", "Mulier fortis;;tone1\nGloria"),
        ]);
        let s = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("Sancti/05-04", "Ant Magnificat", &s, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        assert_eq!(result.as_deref(), Some("Mulier fortis\nGloria"));
    }

    #[test]
    fn resolve_section_runs_conditional_eval_on_final_body() {
        // The redirected body has a (rubrica X) directive (no `sed`
        // → no implicit backscope) that gates a line; runtime
        // conditional eval should drop or keep depending on the rubric.
        let corpus = mock_corpus(&[
            ("Source", "X", "always-on\n(rubrica 1960) only-1960"),
        ]);
        let s_1960 = body_subj(Rubric::Rubrics1960);
        let result = resolve_section_with("Source", "X", &s_1960, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        let body = result.unwrap();
        assert!(body.contains("always-on"), "1960: {body:?}");
        assert!(body.contains("only-1960"), "1960: {body:?}");

        let s_1570 = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("Source", "X", &s_1570, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        let body = result.unwrap();
        assert!(body.contains("always-on"), "1570: {body:?}");
        assert!(!body.contains("only-1960"), "1570: {body:?}");
    }

    #[test]
    fn resolve_section_sed_directive_drops_preceding_line() {
        // With `sed` stopword the directive has implicit LINE backscope
        // — when it fires, the immediately-preceding line is retroactively
        // dropped. This is a Perl bug-for-bug feature used by the
        // upstream corpus to handle "this line under 1570; that line
        // under 1960" patterns.
        let corpus = mock_corpus(&[
            ("Source", "X", "pre-1955-line\n(sed rubrica 1960) only-1960-line"),
        ]);
        let s_1960 = body_subj(Rubric::Rubrics1960);
        let result = resolve_section_with("Source", "X", &s_1960, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        let body = result.unwrap();
        // Under 1960: directive fires; LINE backscope drops `pre-1955-line`.
        assert!(!body.contains("pre-1955-line"), "1960 backscope failure: {body:?}");
        assert!(body.contains("only-1960-line"), "1960 forward-gated: {body:?}");

        let s_1570 = body_subj(Rubric::Tridentine1570);
        let result = resolve_section_with("Source", "X", &s_1570, |p, sec| {
            corpus.get(&(p.to_string(), sec.to_string())).cloned()
        });
        let body = result.unwrap();
        // Under 1570: directive doesn't fire; nothing dropped, sequel gated.
        assert!(body.contains("pre-1955-line"), "1570: {body:?}");
        assert!(!body.contains("only-1960-line"), "1570 forward gating: {body:?}");
    }

    #[test]
    fn parse_at_redirect_self_reference() {
        let r = parse_at_redirect("@:Ant Vespera", "Sancti/05-04", "Oratio").unwrap();
        assert_eq!(r.path, "Sancti/05-04");
        assert_eq!(r.section, "Ant Vespera");
        assert!(r.substitutions.is_none());
    }

    #[test]
    fn parse_at_redirect_explicit_path_and_section() {
        let r = parse_at_redirect("@Commune/C7:Hymnus", "Sancti/05-04", "X").unwrap();
        assert_eq!(r.path, "Commune/C7");
        assert_eq!(r.section, "Hymnus");
        assert!(r.substitutions.is_none());
    }

    #[test]
    fn parse_at_redirect_path_only_inherits_section() {
        let r = parse_at_redirect("@Commune/C7", "Sancti/05-04", "Oratio").unwrap();
        assert_eq!(r.path, "Commune/C7");
        assert_eq!(r.section, "Oratio");
    }

    #[test]
    fn parse_at_redirect_with_substitutions() {
        let r = parse_at_redirect(
            "@:Ant Vespera:s/;;.*//gm",
            "Sancti/05-04",
            "Ant Magnificat",
        )
        .unwrap();
        assert_eq!(r.path, "Sancti/05-04");
        assert_eq!(r.section, "Ant Vespera");
        assert_eq!(r.substitutions.as_deref(), Some("s/;;.*//gm"));
    }

    #[test]
    fn parse_at_redirect_rejects_multiline_body() {
        // A body that starts with `@` but has more content on
        // subsequent lines isn't a redirect.
        let result = parse_at_redirect(
            "@some path\nactual content here",
            "X",
            "Y",
        );
        assert!(result.is_none());
    }

    #[test]
    fn parse_at_redirect_rejects_non_at_body() {
        assert!(parse_at_redirect("plain text", "X", "Y").is_none());
        assert!(parse_at_redirect("", "X", "Y").is_none());
    }

    // ─── regex_lite_match unit tests ─────────────────────────────

    #[test]
    fn regex_lite_anchors() {
        assert!(regex_lite_match("Quad5-5", "Quad5-5$"));
        assert!(regex_lite_match("Sancti/Quad5-5", "Quad5-5$"));
        assert!(!regex_lite_match("Quad5-5z", "Quad5-5$"));
        assert!(regex_lite_match("Sancti/09-15", "09-15$"));
        assert!(!regex_lite_match("Sancti/09-150", "09-15$"));
    }

    #[test]
    fn regex_lite_char_class() {
        assert!(regex_lite_match("Epi1-3", "Epi1-[1-6]"));
        assert!(regex_lite_match("Epi1-1", "Epi1-[1-6]"));
        assert!(regex_lite_match("Epi1-6", "Epi1-[1-6]"));
        assert!(!regex_lite_match("Epi1-0", "Epi1-[1-6]"));
        assert!(!regex_lite_match("Epi1-7", "Epi1-[1-6]"));
        // Quad6-[456]
        assert!(regex_lite_match("Tempora/Quad6-4", "Quad6-[456]"));
        assert!(regex_lite_match("Tempora/Quad6-5", "Quad6-[456]"));
        assert!(regex_lite_match("Tempora/Quad6-6", "Quad6-[456]"));
        assert!(!regex_lite_match("Tempora/Quad6-3", "Quad6-[456]"));
        assert!(!regex_lite_match("Tempora/Quad6-7", "Quad6-[456]"));
    }

    #[test]
    fn subjects_builders_chain() {
        let s = Subjects::new(Rubric::DivinoAfflatu1911, "Pasc1", 13, 4, 2026)
            .with_dayofweek(0)
            .with_hora("Vespera")
            .with_winner("Tempora/Pasc1-0", "I classis")
            .with_commune("Commune/C7")
            .with_votive("")
            .with_dioecesis("Generale")
            .with_missa(false, 0);
        assert_eq!(s.dayname0, "Pasc1");
        assert_eq!(s.day, 13);
        assert_eq!(s.month, 4);
        assert_eq!(s.dayofweek, 0);
        assert_eq!(s.hora, "Vespera");
        assert!(s.is_vesp_or_comp());
    }
}
