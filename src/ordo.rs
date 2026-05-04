//! Mass-Ordinary corpus loader and renderer.
//!
//! Mirrors the Perl walker `propers.pl::specials()` — given the
//! winner's resolved propers + rules, walks one of the per-cursus
//! Ordo templates (`Ordo`, `Ordo67`, `OrdoN`, `OrdoA`, `OrdoM`,
//! `OrdoOP`, `OrdoS`) and emits a flat list of fully-resolved
//! [`RenderedLine`]s. The shipped JSON corpus is built by
//! `data/build_ordo_json.py` from
//! `vendor/divinum-officium/web/www/missa/Latin/Ordo/{Ordo*.txt,
//! Prayers.txt, Prefationes.txt}`.
//!
//! The renderer:
//!   * applies the `(solemn, defunctorum)` mode against `!*FLAG`
//!     guards (`D`, `R`, `S`, `nD`, `RnD`, `SnD`);
//!   * evaluates `!*&hookname` hook-guards against the rules / votive
//!     state — see [`Mode::passes_hook_guard`];
//!   * splices the day's resolved propers (`introitus`, `collect`, …)
//!     into `&propername` insertion points;
//!   * expands `&MacroName` references against the prayer dictionary.
//!
//! The result is consumed by `wasm::compute_mass_full` and emitted as
//! JSON; the demo's `render.js` then turns it into HTML. No Mass
//! Ordinary text lives in JS — it all comes from upstream Perl data
//! transcoded at build time.

use std::collections::HashMap;
use std::sync::OnceLock;

pub use crate::data_types::{OrdoCorpus, OrdoLine};

static ORDO_BR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ordo_latin.postcard.br"));
static PARSED: OnceLock<OrdoCorpus> = OnceLock::new();

/// Decode the embedded Ordo corpus on first access. Postcard +
/// brotli, mirroring [`crate::missa`] and [`crate::sancti`].
pub fn corpus() -> &'static OrdoCorpus {
    PARSED.get_or_init(|| {
        let pc = crate::embed::decompress(ORDO_BR);
        postcard::from_bytes(&pc).expect("ordo postcard decode")
    })
}

/// Look up an Ordo template by name (`Ordo`, `Ordo67`, `OrdoN`,
/// `OrdoA`, `OrdoM`, `OrdoOP`, `OrdoS`).
pub fn template(name: &str) -> Option<&'static [OrdoLine]> {
    corpus().templates.get(name).map(Vec::as_slice)
}

/// Look up a static prayer body by name (`Pater noster`, `Confiteor`,
/// `Gloria`, …). Returns `None` when the macro is unknown — typically
/// indicates a missing `&Foo` reference in the Ordo template.
pub fn prayer(name: &str) -> Option<&'static str> {
    corpus().prayers.get(name).map(String::as_str)
}

/// Look up a preface body by name (`Communis`, `Apostolis`,
/// `Defunctorum`, `Quad`, …). Mirrors the Perl
/// `Ordo/Prefationes.txt` lookup.
pub fn preface(name: &str) -> Option<&'static str> {
    corpus().prefaces.get(name).map(String::as_str)
}

// ─── Render mode ─────────────────────────────────────────────────────

/// Active mode for guard / hook evaluation. Matches the upstream Perl
/// global state at `propers.pl::specials()` entry: `$solemn`,
/// `$votive`, plus the rule flags pulled from the winner's `[Rule]`
/// section.
#[derive(Debug, Clone)]
pub struct Mode {
    /// Solemn vs low Mass. Drives `!*S` / `!*R` guards and `CheckPax`.
    pub solemn: bool,
    /// Active votive — Defunctorum / C9 (Cross) suppress benediction,
    /// Last Gospel substitution, etc.
    pub defunctorum: bool,
    /// Day-of-week (0 = Sunday). Drives `Vidiaquam` (Asperges/Vidi
    /// aquam are sung only on Sundays before the principal Mass).
    pub dayofweek: u8,
    /// Active dayname[0] (`Adv1-0`, `Pasc3-0`, `Quad6-5`, …). Drives
    /// `Pasc` → "Vidi aquam" else "Asperges me", and `DeTemporePassionis`.
    pub dayname: String,
    /// Lower-cased winner `[Rule]` body. Several hook-guards inspect
    /// it: `no Gloria`, `no Credo`, `no Pax`, `no Benedictio`, `no
    /// Ultima Evangelium`, `no Qui Dixisti`.
    pub rule_lc: String,
}

impl Default for Mode {
    fn default() -> Self {
        Self {
            solemn: true,
            defunctorum: false,
            dayofweek: 0,
            dayname: String::new(),
            rule_lc: String::new(),
        }
    }
}

impl Mode {
    /// Evaluate a guard string against this mode. Returns `true` if
    /// the line should be emitted. Mirrors the disjunction in
    /// `propers.pl::specials()` lines 64-83 — each clause sets
    /// `$skipflag = 1` and the line is dropped if any clause fires.
    pub fn passes_guard(&self, guard: &str) -> bool {
        if let Some(hook) = guard.strip_prefix('&') {
            // Hook-guard. Perl: `$skipflag = eval($1)` — block is
            // skipped if the hook returns true.
            return !self.passes_hook_guard(hook);
        }
        // Flag-guards. Order matches Perl's clause order.
        if guard.contains("nD") && self.defunctorum {
            // `!*nD`, `!*RnD`, `!*SnD` are all skipped under Defunctorum.
            return false;
        }
        if guard == "S" && !self.solemn {
            return false;
        }
        if guard == "R" && self.solemn {
            return false;
        }
        if guard == "D" && !self.defunctorum {
            return false;
        }
        // `RnD` = `R` + `nD`: low Mass AND not defunctorum.
        if guard == "RnD" && self.solemn {
            return false;
        }
        if guard == "SnD" && !self.solemn {
            return false;
        }
        true
    }

    /// Evaluate a `!*&hookname` hook-guard. Returns `true` when the
    /// Perl hook would return a truthy value (i.e. when the *block*
    /// should be SKIPPED — the caller must invert).
    fn passes_hook_guard(&self, hook: &str) -> bool {
        match hook {
            // `sub CheckQuiDixisti { our $votive =~ /Defunct|C9/i || our $rule =~ /no Qui Dixisti/i; }`
            "CheckQuiDixisti" => self.defunctorum || self.rule_lc.contains("no qui dixisti"),
            // `sub CheckPax { !(our $solemn) || our $votive =~ /Defunct|C9/i || our $rule =~ /no Pax/i; }`
            "CheckPax" => !self.solemn || self.defunctorum || self.rule_lc.contains("no pax"),
            // `sub CheckBlessing { our $votive =~ /Defunct|C9/i || our $rule =~ /no Benedictio/i; }`
            "CheckBlessing" => self.defunctorum || self.rule_lc.contains("no benedictio"),
            // `sub CheckUltimaEv { our $rule =~ /no Ultima Evangelium/i; }`
            "CheckUltimaEv" => self.rule_lc.contains("no ultima evangelium"),
            // `sub placeattibi { return 0; }` — never skip.
            "placeattibi" => false,
            // Side-effect hooks (`Introibo`, `GloriaM`, `Credo`,
            // `AgnusHook`) appear as `!&hookname`, not `!*&hookname`.
            // If they show up here it's a parser bug; default to "do
            // not skip".
            _ => false,
        }
    }

    /// Whether a `!&Introibo` side-effect hook should emit `omit.
    /// psalm` for this day. Mirrors `sub Introibo` in propers.pl:
    /// fires under Defunctorum / Cross votive / Passiontide.
    pub fn introibo_omits(&self) -> bool {
        if self.defunctorum {
            return true;
        }
        // `DeTemporePassionis` returns true when `dayname[0]` matches
        // `Quad5` / `Quad6` (Passion Sunday through Holy Week).
        let dn = &self.dayname;
        dn.starts_with("Quad5") || dn.starts_with("Quad6")
    }

    /// Whether `!&GloriaM` should emit `omit.` (i.e. Gloria is *not*
    /// sung). Mirrors `sub gloriflag` — too entangled with the rule
    /// for the renderer to re-derive, so we accept the precomputed
    /// `gloria` flag from `wasm.rs::pull_rules`.
    pub fn gloria_emits(&self, gloria_active: bool) -> bool {
        !gloria_active
    }

    /// Whether `!&Credo` should emit `omit.` (i.e. Credo is not sung).
    /// Same precomputed-flag pattern as Gloria.
    pub fn credo_emits(&self, credo_active: bool) -> bool {
        !credo_active
    }
}

// ─── Renderer ────────────────────────────────────────────────────────

/// One emitted line of the Mass Ordinary as fully resolved against
/// the day's office. The renderer collapses macros and propers into
/// concrete bodies; the demo just walks this list and HTML-formats it.
#[derive(Debug, Clone)]
pub enum RenderedLine {
    /// Plain prose (no role marker). Body may carry leading `O.` etc.
    Plain { body: String },
    /// Role-prefixed line: `V`, `R`, `S`, `M`, `D`, `C`, `J`.
    Spoken { role: String, body: String },
    /// Italic rubric. Levels 1/2/3 are Perl `!`/`!!`/`!!!`; 21/22 are
    /// `!x!`/`!x!!` (omitted-comment form, only emitted when `rubrics`
    /// is on).
    Rubric { body: String, level: u8 },
    /// `# Heading` emitted as a section header.
    Section { label: String },
    /// Expanded macro body (multi-line) — already split into role/plain
    /// per line by the macro's source layout. Carries the macro name so
    /// the renderer can offer "from prayer X" provenance.
    Macro { name: String, body: String },
    /// A proper-insertion point (`&introitus`, `&collect`, …). The
    /// `section` is the JSON key under `mass.propers` (`introitus`,
    /// `oratio`, `lectio`, …). Body is left to the JS renderer to look
    /// up — this keeps the rendered list small (no body duplication).
    Proper { section: String },
    /// Side-effect hook output. The walker has already evaluated
    /// whether the hook fires; this variant carries the message that
    /// `propers.pl` would have pushed to `@s` (typically `omit.`,
    /// `omit. psalm`).
    HookOmit { hook: String, message: String },
}

/// Inputs the renderer needs alongside the raw [`Mode`]. Parameters
/// are the precomputed `gloria` / `credo` flags from
/// `wasm::pull_rules` and a `rubrics` toggle (when false, level-1
/// rubrics are suppressed exactly as Perl `propers.pl` line 107).
#[derive(Debug, Clone)]
pub struct RenderArgs<'a> {
    pub mode: &'a Mode,
    pub gloria_active: bool,
    pub credo_active: bool,
    /// Whether the user has rubrics turned on. The Perl renderer
    /// suppresses level-1 rubrics when off but always emits level
    /// >=2 / `!x!!`. Default `true` for the demo.
    pub rubrics: bool,
    /// Template name (`Ordo` for the legacy Tridentine cursus, `Ordo67`
    /// for the 1962, etc.). Derived from the active rubric by the
    /// caller — see `template_name_for_rubric`.
    pub template_name: &'a str,
}

/// Pick the per-cursus Ordo template for a given rubric. Mirrors the
/// dispatch in `Cmissa.pl:52-53`.
pub fn template_name_for_rubric(rubric: crate::core::Rubric) -> &'static str {
    match rubric {
        // 1570 / 1910 / Divino Afflatu / Reduced 1955 / 1960 all share
        // the legacy `Ordo` template (the conditional flags inside the
        // template do the rubric-specific differentiation). Upstream's
        // `Cmissa.pl` only switches to `Ordo67` when version starts
        // with `196*` AND mass-config flag `1962` is set; for the
        // public-facing site we keep `Ordo`.
        _ => "Ordo",
    }
}

/// Walk an Ordo template against a [`Mode`] and produce the
/// fully-resolved Mass Ordinary lines. Macros are inlined, propers
/// are emitted as `RenderedLine::Proper { section }` sentinels for the
/// caller (`wasm.rs`) to splice in the day's resolved bodies.
pub fn render_mass(args: &RenderArgs<'_>) -> Vec<RenderedLine> {
    let template = match template(args.template_name) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let prayers_map: &HashMap<String, String> = &corpus().prayers;
    let mut out = Vec::with_capacity(template.len());

    for line in template {
        // Guard check first — if it fails the line is dropped before
        // any macro / hook / proper handling.
        if let Some(g) = line.guard.as_deref() {
            if !args.mode.passes_guard(g) {
                continue;
            }
        }

        match line.kind.as_str() {
            "blank" => {
                // Blank lines just terminate guards in the source; we
                // don't emit anything for them — the JS renderer adds
                // its own paragraph spacing.
            }
            "section" => {
                if let Some(label) = &line.label {
                    out.push(RenderedLine::Section { label: label.clone() });
                }
            }
            "rubric" => {
                let level = line.level.unwrap_or(1);
                // Perl propers.pl:107 — `if ($item =~ /^\s*!/ &&
                // !$rubrics) { next; }` for level-1; level >=2 always
                // emits.
                if level == 1 && !args.rubrics {
                    continue;
                }
                if level == 21 && !args.rubrics {
                    // `!x!` = omitted-comment form; only when rubrics off.
                    continue;
                }
                if let Some(body) = &line.body {
                    out.push(RenderedLine::Rubric { body: body.clone(), level });
                }
            }
            "spoken" => {
                if let (Some(role), Some(body)) = (&line.role, &line.body) {
                    out.push(RenderedLine::Spoken {
                        role: role.clone(),
                        body: body.clone(),
                    });
                }
            }
            "plain" => {
                if let Some(body) = &line.body {
                    out.push(RenderedLine::Plain { body: body.clone() });
                }
            }
            "macro" => {
                if let Some(name) = &line.name {
                    if let Some(body) = prayers_map.get(name) {
                        out.push(RenderedLine::Macro {
                            name: name.clone(),
                            body: body.clone(),
                        });
                    }
                }
            }
            "proper" => {
                if let Some(name) = &line.name {
                    out.push(RenderedLine::Proper { section: name.clone() });
                }
            }
            "hook" => {
                if let Some(name) = &line.name {
                    if let Some(msg) = run_side_effect_hook(name, args) {
                        out.push(RenderedLine::HookOmit {
                            hook: name.clone(),
                            message: msg,
                        });
                    }
                }
            }
            _ => {
                // Unknown kind — silently drop. Either a future template
                // shape we don't yet model or a parser glitch; the
                // regression suite will surface either case.
            }
        }
    }

    apply_render_scrubs(&mut out);
    out
}

/// Apply the upstream Perl `webdia.pl::display_text` render-time
/// scrubs to every body in a rendered hour. Mirrors the Perl render
/// boundary — see [`crate::scrub::scrub_render_text`]. Mutates in
/// place so callers (Mass and Office walkers) finish their build,
/// then post-process once at the end.
pub fn apply_render_scrubs(lines: &mut [RenderedLine]) {
    for line in lines {
        match line {
            RenderedLine::Plain { body }
            | RenderedLine::Rubric { body, .. }
            | RenderedLine::Macro { body, .. } => {
                let scrubbed = crate::scrub::scrub_render_text(body);
                if scrubbed.as_str() != body.as_str() {
                    *body = scrubbed;
                }
            }
            RenderedLine::Spoken { body, .. } => {
                let scrubbed = crate::scrub::scrub_render_text(body);
                if scrubbed.as_str() != body.as_str() {
                    *body = scrubbed;
                }
            }
            // `Section`/`Proper`/`HookOmit` carry labels/ids/short
            // messages — no scrub needed (they aren't user-visible
            // body prose).
            RenderedLine::Section { .. }
            | RenderedLine::Proper { .. }
            | RenderedLine::HookOmit { .. } => {}
        }
    }
}

/// Side-effect hooks (`!&hookname`) — the Perl callbacks that *push*
/// onto `@s` when fired. We return `Some(msg)` when the hook would
/// have emitted, `None` when silent.
fn run_side_effect_hook(name: &str, args: &RenderArgs<'_>) -> Option<String> {
    match name {
        // sub Introibo { if ($votive =~ /Defunct|C9/ || DeTemporePassionis())
        //                 { push(@s, "!omit. psalm"); return 1; }
        //                 return 0; }
        "Introibo" => args.mode.introibo_omits().then(|| "omit. psalm".to_string()),
        // sub GloriaM { my $flag = gloriflag(); if ($flag) { push(@s, "!omit."); } ... }
        "GloriaM" => (!args.gloria_active).then(|| "omit.".to_string()),
        // sub Credo { my $flag = ...; if ($flag) { push(@s, "!omit."); } ... }
        "Credo" => (!args.credo_active).then(|| "omit.".to_string()),
        // sub AgnusHook — only modifies the previous line if `rule
        // =~ /ter miserere/`. We don't need to do anything from here:
        // the demo renders the Agnus Dei the same either way.
        "AgnusHook" => None,
        _ => None,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_loads_templates_and_prayers() {
        let c = corpus();
        assert!(!c.templates.is_empty(), "templates map empty");
        assert!(c.templates.contains_key("Ordo"), "missing Ordo template");
        assert!(c.prayers.contains_key("Pater noster"));
        assert!(c.prefaces.contains_key("Communis"));
    }

    #[test]
    fn ordo_template_has_propers_in_order() {
        let t = template("Ordo").expect("Ordo template");
        let propers: Vec<&str> = t
            .iter()
            .filter(|l| l.kind == "proper")
            .filter_map(|l| l.name.as_deref())
            .collect();
        // Spot-check the canonical order of the major proper insertions.
        let want = ["introitus", "collect", "lectio", "graduale", "evangelium",
                    "offertorium", "secreta", "prefatio", "communio",
                    "postcommunio", "Ultimaev"];
        for w in want {
            assert!(propers.contains(&w), "missing proper insertion {w}");
        }
    }

    #[test]
    fn flag_guards_match_perl_semantics() {
        let solemn_normal = Mode { solemn: true, defunctorum: false, ..Default::default() };
        let low_normal = Mode { solemn: false, defunctorum: false, ..Default::default() };
        let solemn_defunct = Mode { solemn: true, defunctorum: true, ..Default::default() };

        // `!*S` — solemn-only. Skip under low Mass.
        assert!(solemn_normal.passes_guard("S"));
        assert!(!low_normal.passes_guard("S"));

        // `!*R` — low-only (R = "regular"). Skip under solemn.
        assert!(low_normal.passes_guard("R"));
        assert!(!solemn_normal.passes_guard("R"));

        // `!*D` — Defunctorum-only. Skip otherwise.
        assert!(!solemn_normal.passes_guard("D"));
        assert!(solemn_defunct.passes_guard("D"));

        // `!*nD` — non-Defunctorum-only.
        assert!(solemn_normal.passes_guard("nD"));
        assert!(!solemn_defunct.passes_guard("nD"));
    }

    #[test]
    fn hook_guards_skip_when_hook_returns_true() {
        let mode = Mode {
            rule_lc: "no ultima evangelium".to_string(),
            ..Default::default()
        };
        // CheckUltimaEv returns true → block is SKIPPED → guard fails.
        assert!(!mode.passes_guard("&CheckUltimaEv"));

        let mode_normal = Mode::default();
        // Normal day → CheckUltimaEv returns false → block is emitted.
        assert!(mode_normal.passes_guard("&CheckUltimaEv"));
    }

    #[test]
    fn introibo_omits_in_passiontide() {
        let passion = Mode { dayname: "Quad5-3".to_string(), ..Default::default() };
        assert!(passion.introibo_omits());
        let normal = Mode { dayname: "Pasc3-0".to_string(), ..Default::default() };
        assert!(!normal.introibo_omits());
        let defunct = Mode { defunctorum: true, ..Default::default() };
        assert!(defunct.introibo_omits());
    }

    #[test]
    fn introibo_hook_emits_omit_psalm_under_defunctorum() {
        let mode = Mode {
            solemn: true,
            defunctorum: true,
            dayname: String::new(),
            dayofweek: 0,
            rule_lc: String::new(),
        };
        let args = RenderArgs {
            mode: &mode,
            gloria_active: false,
            credo_active: false,
            rubrics: true,
            template_name: "Ordo",
        };
        let lines = render_mass(&args);
        let hook_count = lines.iter().filter(|l| matches!(l, RenderedLine::HookOmit { hook, .. } if hook == "Introibo")).count();
        assert!(hook_count >= 1, "Introibo should fire under Defunctorum (got {hook_count} HookOmit lines)");
    }

    #[test]
    fn render_mass_strips_upstream_wait_markers() {
        // Ordo.txt:219 carries `wait10 (Jungit manus, …)` inside the
        // first Memento. The renderer must scrub that before emit so
        // the user never sees `wait10` in the rendered Mass.
        let mode = Mode {
            solemn: true,
            defunctorum: false,
            dayname: "Pasc3-0".to_string(),
            ..Default::default()
        };
        let args = RenderArgs {
            mode: &mode,
            gloria_active: true,
            credo_active: true,
            rubrics: true,
            template_name: "Ordo",
        };
        let lines = render_mass(&args);
        // Walk every emitted body and assert the marker is gone.
        for l in &lines {
            let body = match l {
                RenderedLine::Plain { body }
                | RenderedLine::Rubric { body, .. }
                | RenderedLine::Macro { body, .. }
                | RenderedLine::Spoken { body, .. } => body.as_str(),
                _ => continue,
            };
            assert!(
                !body.to_lowercase().contains("wait5"),
                "wait5 leaked into rendered Mass body: {body:?}"
            );
            assert!(
                !body.to_lowercase().contains("wait10"),
                "wait10 leaked into rendered Mass body: {body:?}"
            );
            assert!(
                !body.to_lowercase().contains("wait16"),
                "wait16 leaked into rendered Mass body: {body:?}"
            );
        }
        // Sanity: the Memento line that previously held `wait10`
        // should still emit and contain the (Jungit manus, …) prose.
        let has_memento = lines.iter().any(|l| {
            if let RenderedLine::Spoken { body, .. } = l {
                body.contains("Meménto") && body.contains("Jungit manus")
            } else {
                false
            }
        });
        assert!(has_memento, "Memento with Jungit manus did not survive scrub");
    }

    #[test]
    fn render_mass_emits_propers_and_drops_skipped_blocks() {
        let mode = Mode {
            solemn: true,
            defunctorum: true,
            dayname: "Pasc3-0".to_string(),
            ..Default::default()
        };
        let args = RenderArgs {
            mode: &mode,
            gloria_active: true,
            credo_active: true,
            rubrics: true,
            template_name: "Ordo",
        };
        let lines = render_mass(&args);
        assert!(!lines.is_empty(), "rendered mass empty");
        // Defunctorum: `!*R` blocks (Leonine prayers at end) should be
        // entirely absent.
        let has_leonine = lines.iter().any(|l| matches!(l, RenderedLine::Section { label } if label.contains("Leonis")));
        assert!(!has_leonine, "Leonine prayers should be dropped under Defunctorum (R-only)");
    }
}
