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

/// Walk a section body, dropping lines whose conditional guard is
/// false for the active state.
///
/// Mirror of `SetupString.pl::process_conditional_lines` line 363-474.
/// Lands in B10b-slice-4.
pub fn process_conditional_lines(_body: &str, _subjects: &Subjects<'_>) -> String {
    // TODO(B10b-slice-4): port SetupString.pl:363-474.
    unimplemented!("B10b-slice-4: process_conditional_lines")
}

// ─── Inclusion substitutions (B10b-slice-5) ─────────────────────────

/// Apply `:in N loco s/PAT/REPL/` substitutions on an inclusion. The
/// upstream `@Path:Section in 4 loco s/PAT/REPL/` form pulls the
/// target body, then runs the regex substitution on it.
///
/// Mirror of `SetupString.pl::do_inclusion_substitutions` line 479-493.
/// Lands in B10b-slice-5.
pub fn do_inclusion_substitutions(_body: &mut String, _spec: &str) {
    // TODO(B10b-slice-5): port SetupString.pl:479-493.
    unimplemented!("B10b-slice-5: do_inclusion_substitutions")
}

/// Resolve a load-time `@Path[:Section]` reference. Like the runtime
/// version but applied once at corpus-load time.
///
/// Mirror of `SetupString.pl::get_loadtime_inclusion` line 502-528.
/// Lands in B10b-slice-5.
pub fn resolve_load_time_inclusion(
    _path: &str,
    _section: Option<&str>,
    _substitutions: Option<&str>,
) -> Option<String> {
    // TODO(B10b-slice-5): port SetupString.pl:502-528.
    unimplemented!("B10b-slice-5: resolve_load_time_inclusion")
}

// ─── Top-level resolvers (B10b-slice-6) ─────────────────────────────

/// Top-level section resolver. Mirror of `setupstring` line 534-712.
/// Lands in B10b-slice-6.
pub fn resolve_section(
    _path: &str,
    _section: &str,
    _subjects: &Subjects<'_>,
) -> Option<String> {
    // TODO(B10b-slice-6): port SetupString.pl:534-712.
    unimplemented!("B10b-slice-6: resolve_section (multi-hop @-redirect)")
}

/// Office-side section resolver — adds the per-day commune chain
/// fallback that distinguishes office lookups from Mass lookups.
///
/// Mirror of `SetupString.pl::officestring` line 720-777. Lands in
/// B10b-slice-6.
pub fn resolve_office_section(
    _path: &str,
    _section: &str,
    _subjects: &Subjects<'_>,
) -> Option<String> {
    // TODO(B10b-slice-6): port SetupString.pl:720-777.
    unimplemented!("B10b-slice-6: resolve_office_section")
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
