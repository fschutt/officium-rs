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
//! breviary leg passes `(rubric, dayname, body, …)` explicitly.
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
//! | Perl sub | Lines | Rust target |
//! |---|---|---|
//! | `evaluate_conditional($)` | 118 | [`evaluate_conditional`] |
//! | `conditional_regex()` | 135 | private — pattern compiled once |
//! | `parse_conditional($$$)` | 139 | [`parse_conditional`] |
//! | `get_tempus_id` | 169 | [`get_tempus_id`] |
//! | `get_dayname_for_condition` | 224 | [`dayname_for_condition`] |
//! | `vero($)` | 260 | private predicate evaluator |
//! | `setupstring_parse_file($$$)` | 314 | not ported — done by build script |
//! | `process_conditional_lines` | 363 | [`process_conditional_lines`] |
//! | `do_inclusion_substitutions(\$$)` | 479 | [`do_inclusion_substitutions`] |
//! | `get_loadtime_inclusion($$$$$$$)` | 502 | [`resolve_load_time_inclusion`] |
//! | `setupstring($$%)` | 534 | [`resolve_section`] (re-exported) |
//! | `officestring($$;$)` | 720 | [`resolve_office_section`] |
//! | `checkfile` / `checklatinfile` | 782/821 | not ported — postcard blob always succeeds |

use crate::core::Rubric;

/// Evaluate a conditional fragment from a section body. Returns true
/// when the condition is satisfied under the active rubric / season /
/// dayname.
///
/// Examples:
///   - `(sed rubrica 1960)` — true under Rubric::Rubrics1960.
///   - `(sed rubrica 1955 aut rubrica 1960)` — true under either.
///   - `(nisi rubrica monastica)` — true unless under Monastic.
///   - `(in tempore Adventus)` — true during Advent season.
///
/// Mirror of `SetupString.pl::evaluate_conditional` line 118 plus
/// `vero` line 260.
pub fn evaluate_conditional(_condition: &str, _rubric: Rubric, _dayname: &str) -> bool {
    // TODO(B10): port SetupString.pl:118-313 (~200 LOC). The richest
    // fn in this module — combines pattern matching against the
    // condition body with rubric / season / dayname predicates.
    unimplemented!("phase B10: evaluate_conditional")
}

/// Walk a section body, dropping lines whose conditional guard is
/// false for the active state.
///
/// Mirror of `process_conditional_lines` line 363.
pub fn process_conditional_lines(_body: &str, _rubric: Rubric, _dayname: &str) -> String {
    // TODO(B10): port SetupString.pl:363-478. Today the build script
    // does this baked-1570; runtime port enables proper 5-rubric
    // support without re-baking.
    unimplemented!("phase B10: process_conditional_lines")
}

/// Apply `:in N loco s/PAT/REPL/` substitutions on an inclusion. The
/// upstream `@Path:Section in 4 loco s/PAT/REPL/` form pulls the
/// target body, then runs the regex substitution on it.
///
/// Mirror of `do_inclusion_substitutions` line 479.
pub fn do_inclusion_substitutions(_body: &mut String, _spec: &str) {
    // TODO(B10): port SetupString.pl:479-501. The breviary scope doc
    // flags this as "medium risk" — open question #2.
    unimplemented!("phase B10: do_inclusion_substitutions")
}

/// Resolve a load-time `@Path[:Section]` reference. Like the runtime
/// version but applied once at corpus-load time.
///
/// Mirror of `get_loadtime_inclusion` line 502.
pub fn resolve_load_time_inclusion(
    _path: &str,
    _section: Option<&str>,
    _substitutions: Option<&str>,
) -> Option<String> {
    // TODO(B10): port SetupString.pl:502-533.
    unimplemented!("phase B10: resolve_load_time_inclusion")
}

/// Top-level section resolver. Mirror of `setupstring` line 534.
///
/// Walks the `[Section]` indirection chain:
///   1. Look up `path:section` in the corpus.
///   2. If body starts with `@OtherPath` or `@OtherPath:OtherSection`,
///      recurse with the redirected target (with substitutions
///      applied if present).
///   3. Apply conditional eval to drop rubric-gated lines.
///   4. Return the cleaned body.
///
/// Today's working 1-hop version is in `crate::horas::expand_at_redirect`
/// for Office and `crate::missa::resolve_section` for Mass. After B10
/// both delegate here.
pub fn resolve_section(
    _path: &str,
    _section: &str,
    _rubric: Rubric,
    _dayname: &str,
) -> Option<String> {
    // TODO(B10): port SetupString.pl:534-719 (~185 LOC).
    unimplemented!("phase B10: resolve_section (multi-hop @-redirect)")
}

/// Office-side section resolver — adds the per-day commune chain
/// fallback that distinguishes office lookups from Mass lookups.
///
/// Mirror of `officestring($$;$)` line 720.
pub fn resolve_office_section(
    _path: &str,
    _section: &str,
    _rubric: Rubric,
    _dayname: &str,
) -> Option<String> {
    // TODO(B10): port SetupString.pl:720-781. Wraps `resolve_section`
    // with the office-specific "Psalterium/Special/X" + commune chain
    // fallback that Mass doesn't use.
    unimplemented!("phase B10: resolve_office_section")
}

/// Get the tempus identifier for a section name. Used by
/// `evaluate_conditional` to match `(in tempore X)` clauses.
///
/// Mirror of `get_tempus_id` line 169.
pub fn get_tempus_id(_section: &str) -> Option<String> {
    // TODO(B10): port SetupString.pl:169-223.
    unimplemented!("phase B10: get_tempus_id")
}

/// Map a dayname to the canonical form used by `(...)` conditional
/// fragments. Mirror of `get_dayname_for_condition` line 224.
pub fn dayname_for_condition(_dayname: &str) -> String {
    // TODO(B10): port SetupString.pl:224-259.
    unimplemented!("phase B10: dayname_for_condition")
}

/// Parse a `(sed rubrica X aut Y)` style conditional expression into
/// a structured `Conditional`.
///
/// Mirror of `parse_conditional` line 139.
pub fn parse_conditional(_text: &str) -> Option<Conditional> {
    // TODO(B10): port SetupString.pl:139-168.
    unimplemented!("phase B10: parse_conditional")
}

/// One parsed conditional expression. Used by [`evaluate_conditional`]
/// and [`process_conditional_lines`] to evaluate `(...)` guards.
#[derive(Debug, Clone)]
pub struct Conditional {
    /// The unparsed body of the conditional (everything between `(`
    /// and `)`).
    pub raw: String,
    /// Whether the conditional is `(sed ...)` (positive) or
    /// `(nisi ...)` (negative).
    pub negate: bool,
    /// One or more `rubrica X` / `tempus Y` / `dayname Z` clauses
    /// joined by `aut` (OR) or implicit `et` (AND).
    pub clauses: Vec<ConditionalClause>,
}

/// One clause within a parsed conditional.
#[derive(Debug, Clone)]
pub enum ConditionalClause {
    Rubrica(String),
    Tempus(String),
    Dayname(String),
    Other(String),
}
