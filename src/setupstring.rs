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
/// ## Status
///
/// **B10b-slice-1 stub.** Returns `dayname[0]` unchanged so simple
/// `(... Adv …)` regex-fallback predicates still work for the
/// regression harness. The full season-keyword mapper (Adventus /
/// Nativitatis / Epiphaniæ / post Epiphaniam / Septuagesimæ /
/// Quadragesimæ / Passionis / 8va Paschæ / etc.) lands in B10b-slice-3.
pub fn get_tempus_id(subjects: &Subjects<'_>) -> String {
    // TODO(B10b-slice-3): port SetupString.pl:169-221.
    // For now return dayname[0] — predicates that match against the
    // upstream season tag (`Adv1`, `Quad3`, `Pasc0`, etc.) via the
    // regex-fallback path will still work.
    subjects.dayname0.to_string()
}

/// Get the dayname keyword for `(... die X)` clauses. Mirror of
/// `SetupString.pl::get_dayname_for_condition` line 224-257.
///
/// ## Status
///
/// **B10b-slice-1 stub.** Returns empty string. The full feast-
/// keyword mapper (Epiphaniæ / Tridui Sacri / Omnium Defunctorum /
/// transfigurationis / etc.) lands in B10b-slice-3.
pub fn dayname_for_condition(_subjects: &Subjects<'_>) -> String {
    // TODO(B10b-slice-3): port SetupString.pl:224-257.
    String::new()
}

// ─── Conditional parser (B10b-slice-2 — not yet implemented) ────────

/// Parse a `(sed rubrica X aut Y)` style conditional expression into
/// a structured [`Conditional`].
///
/// Mirror of `SetupString.pl::parse_conditional` line 139-167.
/// Lands in B10b-slice-2.
pub fn parse_conditional(_text: &str) -> Option<Conditional> {
    // TODO(B10b-slice-2): port SetupString.pl:139-167.
    unimplemented!("B10b-slice-2: parse_conditional")
}

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
    /// Whether the condition body itself evaluates true.
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
        // containing /Septua|Quadra|Passio/i. The B10b-slice-1
        // get_tempus_id stub returns dayname[0] verbatim, so we use
        // dayname[0]s that already contain the needed substring.
        // The full slice-3 mapper will translate "Quadp1" → "Septuagesimæ"
        // which also matches.
        let subj = s(Rubric::Tridentine1570, "Septuagesima");
        assert!(vero("post septuagesimam", &subj));

        let subj = s(Rubric::Tridentine1570, "Quadragesima");
        assert!(vero("post septuagesimam", &subj));

        // dayname0 "Adv1" doesn't contain Septua/Quadra/Passio.
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
        // tempore Paschæ → matches "paschali" predicate via named lookup
        let subj = Subjects {
            rubric: Some(Rubric::Tridentine1570),
            dayname0: "Pasc0",
            ..Default::default()
        };
        // dayname0 "Pasc0" — get_tempus_id stub returns "Pasc0";
        // "Pasc0" doesn't contain "Paschæ" / "Ascensionis" /
        // "Octava Pentecostes". So named "paschali" fails.
        // But the regex fallback for "Pasc" (literal) hits. We test
        // the named one via a mocked tempore subject explicitly:
        let subj_tempus = Subjects {
            rubric: Some(Rubric::Tridentine1570),
            // Once get_tempus_id is real (slice 3), `Pasc0` → "Octava Paschæ".
            // For now we test the named predicate against an explicit
            // tempus value via the dayname-fallback path.
            dayname0: "Octava Paschæ",
            ..Default::default()
        };
        assert!(vero("tempore paschali", &subj_tempus));
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
