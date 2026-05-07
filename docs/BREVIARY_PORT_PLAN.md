# Breviary Port Plan (B10 onwards)

**Status (2026-05-07):** Mass-side port is at **100 % Perl-parity
across all 5 rubric layers × 1976-2076** (184,455 / 184,455 days).
Breviary leg is at B7 complete + B8 in progress (see `SUPER_PLAN.md`).
This plan scaffolds the remaining work — B10 through B20 — into a
per-Perl-file/per-subroutine module map and slices it for sequential
execution.

**Read first:** [`MASS_PORT_LESSONS.md`](MASS_PORT_LESSONS.md) — 20
gotchas surfaced during Mass-side cluster closure. Most apply to
the breviary because the upstream Perl shares helper modules
(`Directorium`, `SetupString`, `horascommon`) and data files
(Kalendaria, Transfer/Stransfer, Sancti/, Tempora/, Commune/)
between Mass and Office. Especially relevant for the
precedence/occurrence layer.

This is a **scaffolding plan**, not an implementation. The accompanying
`src/breviary/` skeleton ships with the file/function shape only; every
body is `unimplemented!("phase BNN")` or `todo!()`.

---

## 0. Why a new `src/breviary/` directory

The existing `src/horas.rs` (1947 LOC) is the working B1–B7 code. It
must continue to work — the demo, the WASM build, and `breviary.html`
all wire through it.

But B8+ work needs files an order of magnitude larger than `horas.rs`
can absorb. Following the Mass-side architecture (`src/mass.rs`
4109 LOC + `src/missa.rs` 102 LOC = corpus loader + renderer split),
the breviary leg needs:

- one **corpus-loader** module (today: top of `horas.rs`; future: `src/breviary/corpus.rs`)
- one **renderer** module per upstream Perl chunk
- one **shared cross-cutting** module for utilities both Mass and
  Office consume (today partially in `src/scrub.rs` + `src/missa.rs`;
  future: `src/breviary/setupstring.rs` for the upstream
  `SetupString.pl` 844-LOC behaviour)

The current `src/horas.rs` will be **moved into the new
`src/breviary/` tree** after B10 ships — split function-by-function so
git history survives. Until that move happens, both modules coexist
and `horas.rs` is the sole working entry point. New scaffolding lands
in `src/breviary/` exclusively. See §6 "Migration plan" below.

---

## 1. Upstream Perl files → Rust files

| Upstream file | LOC | Purpose | Rust target |
|---|---|---|---|
| `cgi-bin/horas/officium.pl` | 285 | CGI entry point — collects params, calls `precedence`, drives the per-hour render loop. | `src/wasm.rs::compute_office_full` (already partial) + `src/breviary/officium.rs` (new — pure-functional wrapper). |
| `cgi-bin/horas/horas.pl` | 733 | Top-level renderer — `horas($hora)` outer skeleton, `resolve_refs`, `setlink`, `canticum`, `getordinarium`, `setasterisk`, `postprocess_*`. | `src/breviary/horas.rs` (top-level orchestrator) + `src/breviary/canticum.rs` + `src/breviary/postprocess.rs`. |
| `cgi-bin/horas/horascommon.pl` | 2299 | Lines 1-808 `occurrence` (already in `src/occurrence.rs`); 810-1426 `concurrence` (B11); 1433-1481 `extract_common` (B12); 1502-1512 `gettoday` (in `src/date.rs`); 1527-1838 `precedence` (already in `src/precedence.rs`); 1839-1991 `climit1960` / `setheadline` / `rankname` (B13); 2116-2243 `papal_*` / `gettempora` (B14). | Split: `src/breviary/concurrence.rs`, `src/breviary/setheadline.rs`, `src/breviary/papal.rs`, `src/breviary/gettempora.rs`. |
| `cgi-bin/horas/specials.pl` | 813 | `specials()` walker (lines 21-408) — the heart of the per-hour template processor; rest is helpers (`getproprium`, `getanthoras`, `getantvers`, `setbuild*`, `loadspecial`, `replaceNdot`). | `src/breviary/specials.rs` (walker) + `src/breviary/proprium.rs` (`getproprium`/fallback chain). |
| `cgi-bin/horas/specmatins.pl` | 1857 | Matins-only — `invitatorium`, `hymnusmatutinum`, `nocturn`, `psalmi_matutinum`, `lectiones`, `tedeum_required`, `responsory_gloria`, `ant_matutinum_paschal`, `initiarule`, `resolveitable`, `tferifile`, `StJamesRule`, `prevdayl1`, `contract_scripture`, `getantmatutinum`, `lectio` (ScriptFunc). | `src/breviary/matins/` submodule: `invitatorium.rs`, `nocturn.rs`, `lectiones.rs`, `responsory.rs`, `initiarule.rs`. |
| `cgi-bin/horas/specials/psalmi.pl` | 692 | `psalmi()`, `psalmi_minor`, `psalmi_major`, `antetpsalm`, `get_stThomas_feria`. | `src/breviary/psalter.rs` + `src/breviary/antetpsalm.rs`. |
| `cgi-bin/horas/specials/capitulis.pl` | 255 | `capitulum_major`, `monastic_major_responsory`, `postprocess_short_resp_gabc`, `minor_reponsory`, `capitulum_minor`. | `src/breviary/capitulum.rs`. |
| `cgi-bin/horas/specials/hymni.pl` | 163 | `gethymn`, `hymnusmajor`, `doxology`. | `src/breviary/hymnus.rs`. |
| `cgi-bin/horas/specials/orationes.pl` | 1215 | `oratio` + 8 helpers — collect, commemorations, suffrage, dirge, vigil-commem, papal-prayer, solemn collect. | `src/breviary/oratio.rs` + `src/breviary/suffragium.rs` + `src/breviary/dirge.rs`. |
| `cgi-bin/horas/specials/preces.pl` | 105 | `preces` (predicate), `getpreces` (body emission). | `src/breviary/preces.rs`. |
| `cgi-bin/horas/specials/specprima.pl` | 367 | Prime-only — `lectio_brevis_prima`, `capitulum_prima`, `get_prima_responsory`, `martyrologium`, `luna`, `gregor`. | `src/breviary/prima.rs` + `src/breviary/martyrologium.rs`. |
| `cgi-bin/horas/altovadum.pl` | 1146 | Cistercian-rubric "Altovadensis" overlay — out of scope for first parity pass. | _(deferred)_ — `src/breviary/altovadum.rs` placeholder only. |
| `cgi-bin/horas/monastic.pl` | 675 | Pre-Trident Monastic rubric — out of scope (Mass-side port doesn't ship Monastic either). | _(deferred)_ — `src/breviary/monastic.rs` placeholder only. |
| `cgi-bin/horas/horasjs.pl`, `horasscripts.pl`, `webdia.pl`, `officium_html.pl`, `popup.pl`, `kalendar.pl`, `Cofficium.pl`, `Pofficium.pl`, `appendix.pl` | ~2600 combined | HTML emission, JS embedding, popups, dialog state. None of this ports to a pure-functional Rust core — the WASM consumer renders the structured output instead. | _(no port)_ — formatting layer lives in `demo/render.js` + future `demo/breviary-render.js`. |
| `DivinumOfficium/SetupString.pl` | 844 | **Shared with Mass.** File loader + `setupstring`, `officestring`, `process_conditional_lines`, `evaluate_conditional`, `parse_conditional`, `do_inclusion_substitutions`, `get_loadtime_inclusion`, `checkfile`, `checklatinfile`. Today partially folded into `data/build_horas_json.py` + `data/build_missa_json.py` (build-time conditional eval) and `src/horas.rs::expand_at_redirect` (runtime `@` redirect). | **NEW: `src/setupstring.rs`** — extract a shared module so both Mass and Office consume one resolver. See §3. |
| `DivinumOfficium/RunTimeOptions.pm` | 98 | Active-rubric / language / hour validation. | Not ported — Rust enums (`Rubric`, `Locale`) replace it. |
| `DivinumOfficium/dialogcommon.pl` | 110 | Dialog state — cookies, ini load, runtime options. | Not ported — WASM consumer owns state. |

**Total Perl LOC under `cgi-bin/horas/` = 10,572.** Excluding HTML
emission (~2600 LOC) and the deferred Cistercian/Monastic overlays
(~1820 LOC) leaves **~6150 LOC** of pure rubric / proper-resolution
logic to port. At the Mass-side ratio of 2:1 expansion that's
~12,300 LOC of new Rust — closely matching the Phase-12 estimate in
`BREVIARY_PORT_SCOPE.md`.

---

## 2. Subroutine map

### Already ported (Mass-side or current `horas.rs`)

| Perl sub | Lines | Rust equivalent | Status |
|---|---|---|---|
| `occurrence` | `horascommon.pl:20-808` | `src/occurrence.rs::compute_occurrence` | ✅ |
| `precedence` | `horascommon.pl:1527-1838` | `src/precedence.rs::compute_office` | ✅ |
| `gettoday` | `horascommon.pl:1502` | `src/date.rs` helpers | ✅ |
| `setupstring` (1-hop `@` redirect) | `SetupString.pl:534-720` | `src/horas.rs::expand_at_redirect` (B3 1-hop only) | 🟡 partial |
| `process_conditional_lines` | `SetupString.pl:363-478` | `data/build_horas_json.py` (build-time) | 🟡 partial — runtime eval missing |
| `getproprium` 1-hop fallback | `specials.pl:443-521` | `src/horas.rs::find_section_in_chain` | 🟡 partial — multi-level fallback (winner → commune → tempora → psalterium-special) missing |
| `getordinarium` | `horas.pl:579-602` | `src/horas.rs::compute_office_hour` | ✅ |
| `parse_horas_rank` | `horascommon.pl:1885-1990` (`rankname`) | `src/horas.rs::parse_horas_rank` | ✅ partial |
| First-vespers swap | `horascommon.pl:810-1426` (`concurrence`) | `src/horas.rs::first_vespers_day_key` | 🟡 caller-driven rank compare; full concurrence not ported |
| `replaceNdot` | `specials.pl:782` | `src/horas.rs::substitute_saint_name` | ✅ |
| `commune_chain` (`vide CXX` walker) | `specials.pl:443-521` (embedded) | `src/horas.rs::commune_chain` + `parse_vide_targets` | ✅ |

### Net-new subs to port (B10+)

| Perl sub | Lines | Target Rust | Slice |
|---|---|---|---|
| `concurrence` (full) | `horascommon.pl:810-1426` | `src/breviary/concurrence.rs::compute_concurrence` | B11 |
| `extract_common` | `horascommon.pl:1433-1480` | `src/breviary/concurrence.rs::extract_common` | B11 |
| `setsecondcol` | `horascommon.pl:1512-1525` | folded into `src/breviary/horas.rs::compute_office_hour` (single-column always) | B10 |
| `setheadline` / `rankname` | `horascommon.pl:1868-1990` | `src/breviary/setheadline.rs` | B13 |
| `gettempora` | `horascommon.pl:2243+` | `src/breviary/gettempora.rs` | B14 |
| `papal_rule` / `papal_prayer` / `papal_antiphon_dum_esset` | `horascommon.pl:2175-2243` | `src/breviary/papal.rs` | B14 |
| `specials` (walker) | `specials.pl:21-408` | `src/breviary/specials.rs::run_specials_walker` | B10 |
| `getproprium` (full 4-level fallback) | `specials.pl:443-521` | `src/breviary/proprium.rs::get_proprium` | B10 |
| `getanthoras` / `getantvers` | `specials.pl:543-639` | `src/breviary/proprium.rs::get_ant_hours` / `get_ant_vers` | B10 |
| `getfrompsalterium` | `specials.pl:640-657` | `src/breviary/proprium.rs::get_from_psalterium` | B10 |
| `loadspecial` | `specials.pl:769` | `src/breviary/specials.rs::load_special` | B10 |
| `checksuffragium` | `specials.pl:700-768` | `src/breviary/suffragium.rs::check_suffragium` | B12 |
| `psalmi`, `psalmi_minor`, `psalmi_major`, `antetpsalm`, `get_stThomas_feria` | `specials/psalmi.pl` (full) | `src/breviary/psalter.rs::*` + `src/breviary/antetpsalm.rs::format_antiphon_psalm` | B15 (largest single slice) |
| `gethymn` / `hymnusmajor` / `doxology` | `specials/hymni.pl` (full) | `src/breviary/hymnus.rs::*` | B16 |
| `capitulum_major` / `capitulum_minor` / `monastic_major_responsory` / `minor_reponsory` / `postprocess_short_resp_gabc` | `specials/capitulis.pl` (full) | `src/breviary/capitulum.rs::*` | B16 |
| `oratio` + 8 helpers | `specials/orationes.pl` (full) | `src/breviary/oratio.rs::*` + `src/breviary/suffragium.rs::*` + `src/breviary/dirge.rs::*` | B17 (densest slice — 1215 LOC Perl) |
| `preces` / `getpreces` | `specials/preces.pl` (full) | `src/breviary/preces.rs::*` | B12 |
| `lectio_brevis_prima` / `capitulum_prima` / `get_prima_responsory` / `martyrologium` / `luna` / `gregor` | `specials/specprima.pl` (full) | `src/breviary/prima.rs::*` + `src/breviary/martyrologium.rs::*` | B18 |
| `invitatorium` / `hymnusmatutinum` / `nocturn` / `psalmi_matutinum` / `lectiones` / `getantmatutinum` / `responsory_gloria` / `ant_matutinum_paschal` / `tedeum_required` / `initiarule` / `resolveitable` / `tferifile` / `StJamesRule` / `prevdayl1` / `contract_scripture` / `lectio_brevis_prima` / `lectio` (ScriptFunc) | `specmatins.pl` (full) | `src/breviary/matins/{invitatorium,hymnus,nocturn,psalmody,lectiones,initiarule,responsory}.rs` | B19 (Matins is its own slice — `specmatins.pl` is 1857 LOC and the densest hour) |
| `canticum` / `ant123_special` | `horas.pl:472-569` | `src/breviary/canticum.rs::*` | B16 |
| `resolve_refs` / `adjust_refs` / `setlink` / `get_link_name` / `Septuagesima_vesp` / `triduum_gloria_omitted` / `getantcross` / `depunct` / `setasterisk` / `columnsel` / `postprocess_ant` / `postprocess_vr` / `postprocess_short_resp` / `alleluia_required` | `horas.pl` (rest) | `src/breviary/postprocess.rs` (most) + `src/breviary/triduum.rs` (gloria-omission helpers) | B20 |
| `adhoram` / `horas` (top wrapper) | `horas.pl:18-84` | `src/breviary/horas.rs::compute_office_hour` (already partial) | B10 |

---

## 3. Shared / cross-cutting subs

The Mass leg has been the more advanced port to date, so the
shared helpers currently sit under Mass-flavoured names. **B10 should
extract these into shared modules first** so the breviary work
doesn't duplicate them.

### Candidates for extraction into a new `src/setupstring.rs`

These are all called by both `cgi-bin/missa/` and `cgi-bin/horas/`:

| Perl helper | Today's Rust home | Proposed shared home |
|---|---|---|
| `setupstring(lang, file)` (parse `[Section] body` grammar) | duplicated in `data/build_missa_json.py` + `data/build_horas_json.py` (build-time) and `src/missa.rs::resolve_section` + `src/horas.rs::find_section_in_chain` (runtime) | `src/setupstring.rs::parse_sections` (build-time) + `src/setupstring.rs::resolve_section` (runtime) |
| `process_conditional_lines` (`(sed rubrica X)` evaluator) | build-time only in `build_*_json.py`; runtime evaluation missing | `src/setupstring.rs::evaluate_conditional` (runtime, rubric-aware) |
| `do_inclusion_substitutions` (`@:Section in N loco s/PAT/REPL/`) | not ported — `src/horas.rs::expand_at_redirect` handles 1-hop only | `src/setupstring.rs::expand_inclusion` |
| `evaluate_conditional` / `vero` / `parse_conditional` | not ported (build script bakes 1570 only) | `src/setupstring.rs::eval_conditional` (runtime, all 5 rubrics) |
| `get_loadtime_inclusion` | not ported | `src/setupstring.rs::resolve_load_time_inclusion` |

**Decision:** B10 ships `src/setupstring.rs` as the canonical home for
all `SetupString.pl` behaviour. Both `src/mass.rs` and the new
`src/breviary/*.rs` files call into it. The build scripts are
unchanged but the runtime gains a proper conditional / inclusion
evaluator — needed for the breviary `@:Section in 4 loco s/PAT/REPL/`
form that the scope doc flags as "medium risk" (open question #2).

### Already-shared helpers (no movement needed)

| Helper | Home | Used by |
|---|---|---|
| `Date` arithmetic, `easter`, `getweek`, leap-shift | `src/date.rs` | Mass + Office |
| `compute_occurrence` | `src/occurrence.rs` | Mass + Office (via `precedence`) |
| `compute_office` | `src/precedence.rs` | Mass + Office |
| `Rank`, `RankClass`, `Season`, `Color`, `DayKind`, `OfficeOutput`, `FileKey` | `src/core.rs` | Mass + Office |
| Sancti loader + transfer table + tempora redirects | `src/sancti.rs`, `src/transfer_table.rs`, `src/tempora_table.rs` | Mass + Office |
| `prayers::lookup` (Pater/Gloria/Ave macros) | `src/prayers.rs` | Mass + Office |
| `scrub_render_text` | `src/scrub.rs` | Mass + Office (Office calls it via `apply_render_scrubs`) |
| Postcard corpus loader (combined `corpus.postcard.br`) | `src/embed.rs` | Mass + Office |

---

## 4. Slice plan: B10 – B20

Each slice has: scope, dependencies, estimated complexity (S/M/L/XL),
the Perl line range it ports, and the target Rust files.

### B10 — `setupstring` runtime + `OfficeInput` config + scaffolding (M, M)

**Scope:** This slice is the foundation: ship the runtime
`setupstring` evaluator (functional-style port of `SetupString.pl`),
add the new `OfficeInput` / `OfficeOutput` config fields, and land
the `src/breviary/` module tree (already scaffolded in this PR;
B10 fills the bodies of the smaller helpers).

**B10a — `OfficeInput` / `OfficeOutput` config additions** (per §7.4):

- Add to `src/core.rs`:
  - `pub psalmvar: bool` on `OfficeInput` and `OfficeOutput`.
  - `pub votive: Option<VotiveKind>` on both.
  - `pub cursus: Cursus` on both.
  - New `enum VotiveKind` and `enum Cursus`.
- Update every constructor call site (regression harness, demo,
  WASM API, Mass-side tests) to pass `psalmvar: false`,
  `votive: None`, `cursus: Cursus::Roman`. The existing
  Mass-side behaviour is unchanged because all three default to
  the "neutral" value.
- `compute_office` (`src/precedence.rs`) gets a pre-check: when
  `votive == Some(_)`, route to a votive resolver stub
  (`unimplemented!("phase B10b")` for non-Roman cursus).
- Tests: every Mass-side test still passes; one new test confirms
  `votive: Some(Defunctorum)` panics at the stub (proof the route
  fires).

**B10b — runtime `setupstring` port** (per §7.1):

- Port `SetupString.pl::evaluate_conditional`, `vero`,
  `parse_conditional`, `process_conditional_lines`,
  `do_inclusion_substitutions`, `get_loadtime_inclusion`,
  `setupstring`, `officestring` into `src/setupstring.rs`.
- All in functional style: no thread-locals, no globals; each
  function pure over `(rubric, dayname, body, …)`.
- Re-point `src/missa.rs::resolve_section` and
  `src/horas.rs::expand_at_redirect` to delegate to the new
  resolver. Keep both shims as 1-call thin wrappers so existing
  call sites compile unchanged.
- Tests: round-trip every multi-hop redirect in the existing corpus
  (a corpus-walking test that confirms zero `@`-leaks in any
  resolved section under any of the 5 rubrics).

**B10c — Scaffolding finalisation:**

- The `src/breviary/` tree shipped with this scaffolding PR. B10c
  upgrades the smaller stub bodies (`Hour::parse`,
  `Hour::ordinarium_filename`, `Hour::ALL`, `ad_horam_heading`,
  `resolve_psalter_variant`) from `unimplemented!()` to working
  bodies. The non-trivial slices (B11+) stay
  `unimplemented!()`.

**Perl lines ported:** `SetupString.pl` (~600 of 844 LOC; the rest
is the build-time parser, already replicated in
`data/build_horas_json.py`).
**Files touched:** `src/core.rs` (config fields), `src/setupstring.rs`
(real impls), `src/missa.rs` + `src/horas.rs` (delegate),
`src/breviary/horas.rs` (small body fills).
**Dependencies:** none.
**Estimated Rust LOC:** ~600 in `setupstring.rs` + ~80 across
`core.rs`/`precedence.rs` for the config fields.

### B11 — Concurrence + first-vespers (full) (L, M)

**Scope:** Port `concurrence()` from `horascommon.pl:810-1426` to
produce `OfficeOutput.vespers_split: Some(VespersSplit { … })`.

- `src/breviary/concurrence.rs::compute_concurrence(today: &OfficeOutput, tomorrow: &OfficeOutput) -> Option<VespersSplit>`
- `src/breviary/concurrence.rs::extract_common(…)` (line 1433-1480)
- Drives `vespers: 1|3` selection + `cwinner` rotation + ACapitulo
  branch (`flcrank == flrank`).
- Test cases: Sat→Sun Vespers, Dec 31 → Jan 1, Saturday→Saturnine
  BVM, Pasc6-6→Pasc0, Pent6-6→Pent0.

**Perl lines ported:** `horascommon.pl:810-1480` (~670 LOC).
**Files touched:** `src/breviary/concurrence.rs`,
`src/precedence.rs` (wire `vespers_split` from new helper).
**Dependencies:** B10.
**Estimated Rust LOC:** ~600.

### B12 — Preces + Suffragium predicates (S, S)

**Scope:** Port `preces`/`getpreces` from `specials/preces.pl` and
`checksuffragium` from `specials.pl:700-768`.

- `src/breviary/preces.rs::should_say_preces(office, hour) -> PrecesKind`
- `src/breviary/preces.rs::get_preces_body(hour, kind, …) -> Vec<RenderedLine>`
- `src/breviary/suffragium.rs::should_say_suffragium(office) -> bool`
- `src/breviary/suffragium.rs::get_suffragium_body(office) -> Vec<RenderedLine>`

**Perl lines ported:** `specials/preces.pl` (105) + `specials.pl:700-768` (~70).
**Files touched:** `src/breviary/preces.rs`, `src/breviary/suffragium.rs`.
**Dependencies:** B10.
**Estimated Rust LOC:** ~250.

### B13 — Headline + rankname (S, S)

**Scope:** `setheadline` / `rankname` produce the per-day banner that
the demo currently builds in JS. Porting the Perl logic into Rust
lets us drop the JS reimplementation.

**Perl lines ported:** `horascommon.pl:1868-1991` (~120 LOC).
**Files touched:** `src/breviary/setheadline.rs`.
**Dependencies:** B10.
**Estimated Rust LOC:** ~150.

### B14 — Papal commemorations + gettempora (M, S)

**Scope:** Pope-of-the-day antiphons, papal-rule parser, tempora
keyword resolution.

- `src/breviary/papal.rs::papal_rule(rule: &str) -> Option<PapalClass>`
- `src/breviary/papal.rs::papal_antiphon_dum_esset(office)`
- `src/breviary/gettempora.rs::get_tempora(section_key, office) -> Option<&str>`

**Perl lines ported:** `horascommon.pl:2116-2250` (~140 LOC).
**Files touched:** `src/breviary/papal.rs`, `src/breviary/gettempora.rs`.
**Dependencies:** B10.
**Estimated Rust LOC:** ~250.

### B15 — Psalter (psalmi_minor / psalmi_major) (XL, L)

**Scope:** The single largest slice after Matins. Implements the
full weekly psalter — Roman + Pius X + Paschal + Monastic / Cistercian
overrides for Lauds, Vespers, Prime, Tertia, Sexta, Nona, Compline.

- `src/breviary/psalter.rs::psalmi_minor(office, hour)`
- `src/breviary/psalter.rs::psalmi_major(office, hour)`
- `src/breviary/psalter.rs::psalmi(office, hour)` (dispatch)
- `src/breviary/antetpsalm.rs::format_antiphon_psalm(ant, psalm_num, …)`
- `src/breviary/psalter.rs::get_st_thomas_feria(date)` (Saturday-of-Advent
  3 Thomas-the-Apostle pre-empt)

**Perl lines ported:** `specials/psalmi.pl` (692 LOC).
**Files touched:** `src/breviary/psalter.rs`,
`src/breviary/antetpsalm.rs`.
**Dependencies:** B10. Strictly speaking does not need B11–B14, but
testing this against the Perl oracle will surface bugs in those.
**Estimated Rust LOC:** ~1100.

### B16 — Hymnus + Capitulum + Canticum (L, M)

**Scope:** Three medium chunks merged into one slice because they
share the rubric-conditional doxology / per-season override logic.

- `src/breviary/hymnus.rs::get_hymn(office, hour)`
- `src/breviary/hymnus.rs::doxology(office, hour) -> Option<&str>`
- `src/breviary/capitulum.rs::capitulum_major(office, hour)`
- `src/breviary/capitulum.rs::capitulum_minor(office, hour)`
- `src/breviary/capitulum.rs::short_responsory(office, hour)`
- `src/breviary/canticum.rs::canticum(item, office, hour) -> Vec<RenderedLine>`
- `src/breviary/canticum.rs::ant123_special(office, hour)` (Advent
  O-antiphons + Pope-Confessor Magnificat antiphon)

**Perl lines ported:** `specials/hymni.pl` (163) +
`specials/capitulis.pl` (255) + `horas.pl:472-569` (~100). Total ~520.
**Files touched:** `src/breviary/hymnus.rs`,
`src/breviary/capitulum.rs`, `src/breviary/canticum.rs`.
**Dependencies:** B10, B14.
**Estimated Rust LOC:** ~900.

### B17 — Oratio + Commemorations + Suffragium body (XL, L)

**Scope:** The biggest single Perl file (1215 LOC). Drives the collect
+ all per-day commemorations + Suffragium of All Saints + dirge of
the dead + papal-prayer insertion.

- `src/breviary/oratio.rs::oratio(office, lang, month, day, params) -> Vec<RenderedLine>`
- `src/breviary/oratio.rs::check_commemoratio(…)`
- `src/breviary/oratio.rs::del_conclusio(…)`
- `src/breviary/oratio.rs::get_commemoratio(…)`
- `src/breviary/oratio.rs::vigilia_commemoratio(…)`
- `src/breviary/oratio.rs::get_refs(…)`
- `src/breviary/oratio.rs::oratio_solemnis(…)`
- `src/breviary/suffragium.rs::get_suffragium_body(…)` (full body emission)
- `src/breviary/dirge.rs::dirge(date, hour)` (uses
  `DivinumOfficium::Directorium::dirge`)

**Perl lines ported:** `specials/orationes.pl` (1215 LOC).
**Files touched:** `src/breviary/oratio.rs`, `src/breviary/dirge.rs`,
`src/breviary/suffragium.rs` (extends B12).
**Dependencies:** B10, B12, B14.
**Estimated Rust LOC:** ~1400.

### B18 — Prima specials + Martyrologium (M, M)

**Scope:** Prime is its own dense hour — martyrology block, lectio
brevis, "De Officio Capituli" (1960 only), Regula (Monastic), separate
conclusion.

- `src/breviary/prima.rs::lectio_brevis_prima(office)`
- `src/breviary/prima.rs::capitulum_prima(office, has_responsorium)`
- `src/breviary/prima.rs::get_prima_responsory(office)`
- `src/breviary/martyrologium.rs::martyrologium(date, lang) -> String`
- `src/breviary/martyrologium.rs::luna(date) -> u8` (golden number)
- `src/breviary/martyrologium.rs::gregor(date) -> &'static str` (DiPippo
  paschal-shift table — preferred over Perl when they diverge)

**Perl lines ported:** `specials/specprima.pl` (367 LOC).
**Files touched:** `src/breviary/prima.rs`,
`src/breviary/martyrologium.rs`.
**Dependencies:** B10, B16.
**Estimated Rust LOC:** ~700, plus a new `data/build_martyrologium_json.py`
(~250 Python LOC) feeding `data/martyrologium_latin.json`.

### B19 — Matins (specmatins.pl) (XL, XL)

**Scope:** The single biggest hour. `specmatins.pl` is 1857 LOC of
Matins-specific logic.

- `src/breviary/matins/mod.rs` — entry / dispatch
- `src/breviary/matins/invitatorium.rs::invitatorium(office)`
- `src/breviary/matins/hymnus.rs::hymnus_matutinum(office)`
- `src/breviary/matins/nocturn.rs::nocturn(office, n: u8)`
- `src/breviary/matins/psalmody.rs::psalmi_matutinum(office)`
- `src/breviary/matins/psalmody.rs::ant_matutinum_paschal(office)`
- `src/breviary/matins/psalmody.rs::get_ant_matutinum(office, n: u8)`
- `src/breviary/matins/lectiones.rs::lectiones(office)` — the 9-lectio
  splice (B5 has a basic version in `horas.rs`; this is the rubric-aware
  full port)
- `src/breviary/matins/lectiones.rs::matins_lectio_responsory_alleluia(…)`
- `src/breviary/matins/lectiones.rs::contract_scripture(…)`
- `src/breviary/matins/responsory.rs::responsory_gloria(…)`
- `src/breviary/matins/initiarule.rs::initiarule(office)` —
  scripture-rotation table (when displaced by feasts, what shifts where)
- `src/breviary/matins/initiarule.rs::resolveitable(…)`
- `src/breviary/matins/initiarule.rs::tferifile(…)`
- `src/breviary/matins/initiarule.rs::st_james_rule(…)`
- `src/breviary/matins/initiarule.rs::prevday_l1(…)`
- `src/breviary/matins/lectiones.rs::tedeum_required(office) -> bool`
- `src/breviary/matins/lectiones.rs::get_absolutio_et_benedictiones(…)`

**Perl lines ported:** `specmatins.pl` (1857 LOC).
**Files touched:** new `src/breviary/matins/` directory.
**Dependencies:** B10, B14, B15, B16.
**Estimated Rust LOC:** ~2200.

### B20 — Postprocess + render orchestration (M, M)

**Scope:** The remaining `horas.pl` helpers — text postprocessing
(antiphon dot, Alleluia injection, dagger placement, mute-vowel
italics), spec-walker glue, and the final `compute_office_full`
top-level Rust API that callers (WASM, regression harness) hit.

- `src/breviary/postprocess.rs::postprocess_ant(ant: &mut String, lang)`
- `src/breviary/postprocess.rs::postprocess_vr(…)`
- `src/breviary/postprocess.rs::postprocess_short_resp(capit: &mut Vec<…>, lang)`
- `src/breviary/postprocess.rs::resolve_refs(text, lang)`
- `src/breviary/postprocess.rs::adjust_refs(name, lang, office)`
- `src/breviary/postprocess.rs::ensure_single_alleluia(…)`
- `src/breviary/postprocess.rs::ensure_double_alleluia(…)`
- `src/breviary/postprocess.rs::set_asterisk(line)` (psalm-verse
  formatting)
- `src/breviary/postprocess.rs::get_ant_cross(psalm_line, ant_line)`
  (Tridentine dagger marker)
- `src/breviary/triduum.rs::triduum_gloria_omitted(office) -> bool`
- `src/breviary/triduum.rs::septuagesima_vesp(office, hour) -> bool`
- `src/breviary/horas.rs::compute_office_hour_full(office, hour) -> Office`
  — final orchestrator, replacing `src/horas.rs::compute_office_hour`.

**Perl lines ported:** rest of `horas.pl` (~430 LOC) + glue.
**Files touched:** `src/breviary/postprocess.rs`,
`src/breviary/triduum.rs`, `src/breviary/horas.rs`,
`src/wasm.rs` (cut over the WASM API).
**Dependencies:** B10–B19.
**Estimated Rust LOC:** ~600.

### Slice complexity totals

| Slice | Rust LOC est. | Perl LOC ported | Complexity |
|---|---|---|---|
| B10 | ~700 (foundation) | 600 | M / M |
| B11 | ~600 | 670 | L / M |
| B12 | ~250 | 175 | S / S |
| B13 | ~150 | 120 | S / S |
| B14 | ~250 | 140 | M / S |
| B15 | ~1100 | 692 | XL / L |
| B16 | ~900 | 520 | L / M |
| B17 | ~1400 | 1215 | XL / L |
| B18 | ~700 | 367 | M / M |
| B19 | ~2200 | 1857 | XL / XL |
| B20 | ~600 | 430 | M / M |
| **Total** | **~8850** | **~6786** | — |

The 8850 LOC sits inside the scope-doc estimate of 11 000 – 15 000 LOC
because the existing `horas.rs` is already counted (1947 LOC done) and
the regression harness extension (~1500 LOC) lands as part of leg-K /
the year-sweep tooling, not the breviary slices themselves.

---

## 5. Test strategy

Per slice, tests pile up in three layers (mirroring the Mass-side
pattern):

1. **Unit tests** in the same file, exercising pure helpers
   (`#[cfg(test)] mod tests`).
2. **Per-slice integration tests** in `tests/breviary_<sliceid>.rs`
   that exercise the public API of the slice against a handful of
   hand-picked dates.
3. **Year-sweep regression** under the existing `regression.rs`
   harness — gated behind the `regression` feature, compares Rust
   output against the Perl oracle bytewise.

The `regression.rs` harness today runs Mass cells only. Extending it
to the 8 breviary hours × 5 rubrics × 365 days = 14 600 cells per
rubric is itself a chunk of work — slated for **B21** (year-sweep
extension) which sits outside this scaffolding plan and is tracked
in `SUPER_PLAN.md` as part of leg-K.

---

## 6. Migration plan: `src/horas.rs` → `src/breviary/`

The existing `src/horas.rs` is 1947 LOC. After B11 is in place this
file should be split function-by-function into the new tree, in this
order (each row is one PR; do not bundle):

| Day | Source | Target | Tests to keep green |
|---|---|---|---|
| M1 | `src/horas.rs::lookup`, `iter`, `psalm`, corpus loaders | `src/breviary/corpus.rs` | `corpus_loads_some_horas_files`, `psalm_1_has_latin_body` |
| M2 | `src/horas.rs::OfficeArgs`, `compute_office_hour` (signature only) | `src/breviary/horas.rs::compute_office_hour` | all `compute_office_hour_*` tests |
| M3 | `commune_chain`, `parse_vide_targets`, `tempora_sunday_fallback`, `find_section_in_chain`, `expand_at_redirect`, `looks_like_corpus_path`, `first_path_token` | `src/breviary/proprium.rs` | `commune_chain_*`, `parse_vide_targets_*`, `expand_at_redirect_*` |
| M4 | `expand_dollar_macro`, `lookup_horas_macro`, `resolve_self_redirect` | `src/breviary/macros.rs` | (covered by `compute_office_hour_*` tests) |
| M5 | `splice_proper_into_slot`, `slot_candidates`, `splice_matins_lectios`, `collect_nocturn_antiphons`, `parse_antiphon_lines`, `emit_nocturn_antiphon_block`, `strip_te_deum_directive`, `lookup_te_deum_body` | `src/breviary/specials.rs` + `src/breviary/matins/lectiones.rs` | `matutinum_*`, `splice_*` |
| M6 | `parse_horas_rank`, `first_vespers_day_key` | `src/breviary/concurrence.rs` (exists from B11) | `parse_horas_rank_*`, `first_vespers_day_key_*` |
| M7 | `substitute_saint_name`, `rule_lectio_count` | `src/breviary/postprocess.rs` (exists from B20) | `substitute_saint_name_*` |
| M8 | Delete `src/horas.rs`. Re-point `lib.rs` to `pub mod breviary;` only. Preserve a `src/horas.rs` deprecation stub that re-exports `crate::breviary::*` for one release cycle. | — | full test suite green |

Sequencing: M1–M2 require B10 (the breviary skeleton must exist).
M3–M8 happen **after** the corresponding slice (B11 ⇒ M6, B19 ⇒ M5,
etc.) so the target file already has scaffolding to absorb the
moved code.

---

## 7. Resolved decisions (2026-05-06)

These were open questions in the first draft of this plan; resolved
in the planning thread before B10 begins. They're recorded here as
fixed contract for the leg.

### 7.1 — `setupstring` is a runtime Rust port (functional style)

The conditional evaluator (`(sed rubrica X)`, `(in tempore Adventus)`,
etc.), `@`-redirect chasing, and `:in N loco s/PAT/REPL/` substitution
all port from Perl into pure Rust functions in `src/setupstring.rs`.
**No build-time baking** of rubric variants and **no shipping of
five copies** of the corpus.

Functional-style means: every helper is a pure function over its
inputs (active rubric, dayname, body) returning a fresh `String` or
`bool`. No globals; no `thread_local!` ambient state; no mutation of
the corpus blob. The `Conditional` / `ConditionalClause` types in
`setupstring.rs` are the typed AST — parse once, evaluate per call.

This was the working assumption of B10 in the original plan; now
confirmed.

### 7.2 — Variants are config struct fields, not feature flags

All optional renderer behaviour is **input-config arguments** carried
through `OfficeInput` (or, for purely render-time toggles, a separate
`RenderConfig` struct passed to the per-hour entry point). No Cargo
`--features` knobs for rubric variants — the same compiled binary
must serve every (rubric × variant) tuple at runtime so the WASM
build exposes them all.

The first parity pass adds these fields to `OfficeInput` (B10
prerequisite — see §7.4 for `votive`):

```rust
pub struct OfficeInput {
    pub date: Date,
    pub rubric: Rubric,
    pub locale: Locale,
    pub is_mass_context: bool,
    // ─── B10 additions ──────────────────────────────────────
    /// Pius XII / Bea revision of the psalter (`Latin-Bea`).
    /// Default false — Vulgate text. When true the renderer
    /// substitutes `PsalmFile::latin_bea` for `PsalmFile::latin`
    /// in every psalm body.
    pub psalmvar: bool,
    /// Votive office override — short-circuits the precedence
    /// engine and pulls text from `CommuneM/Cxx*.txt`. `None` =
    /// regular calendar-of-the-day.
    pub votive: Option<VotiveKind>,
    /// Cursus selector. `Roman` is the only fully-implemented
    /// variant during the first parity pass; `Monastic` and
    /// `Cisterciensis` route through stubbed entry points that
    /// `unimplemented!()` until those rubrics are ported.
    pub cursus: Cursus,
}

pub enum VotiveKind {
    Defunctorum,    // Office of the Dead   (CommuneM/C9*.txt)
    BmvParva,       // Little Office of BVM (CommuneM/C12*.txt)
    DeBeataMaria,   // Saturday Office of BVM
    DeAngelis,      // Tuesday Office of Angels
    DeApostolis,    // Wednesday Office of Apostles
    DePassione,     // Friday Office of the Passion
    SsTrinitate,    // Office of the Holy Trinity
    SsSacramento,   // Office of the Blessed Sacrament
}

pub enum Cursus {
    Roman,           // ✅ first parity pass
    Monastic,        // ⏳ stub only — config flag exists; impl panics
    Cisterciensis,   // ⏳ stub only — config flag exists; impl panics
    Praedicatorum,   // ⏳ stub only — config flag exists; impl panics
}
```

The same pattern applies to render-time toggles that don't change
rank/precedence (lang2 second-column, GABC chant tones, build-script
trace verbosity): they live on a sibling `RenderConfig` struct, not on
`OfficeInput`. **B20 lands `RenderConfig`**; B10 only needs the three
`OfficeInput` fields above.

### 7.3 — Monastic / Cistercian / Praedicatorum: input-config only, stubs

Confirmed: phase 1 (B10–B20) ships **Roman only**. The other cursus
values (`Cursus::Monastic`, `Cursus::Cisterciensis`,
`Cursus::Praedicatorum`) are accepted at the API surface but every
code path that branches on them lands in an `unimplemented!()` body
inside `src/breviary/monastic.rs` and `src/breviary/altovadum.rs`.

This is intentional: the API surface (the input struct, the renderer
signature) must be stable across what the user opts into. Adding the
real Monastic / Cistercian implementations later is a code-only
change — no caller has to grow new arguments.

The stubs panic loudly at runtime so a stray Monastic call against
the first-parity build is unmissable. The Roman path is the only one
exercised by tests until those rubrics get their own slice (post-B20,
not in this plan).

This is also called out as the **last bullet in §8 ("Out of scope")**
so the consumer-facing constraint is explicit.

### 7.4 — `OfficeInput::votive` lands as a `Option<VotiveKind>` field

The `Option<VotiveKind>` field on `OfficeInput` (shown in §7.2) is
the canonical home for the votive-office toggle. `compute_office`
(`src/precedence.rs`) reads it; when `Some(_)`, the function
short-circuits the regular precedence engine and routes to a
votive-specific resolver that pulls text from `CommuneM/Cxx*.txt`.
The resolved `OfficeOutput` carries the votive choice on a parallel
`votive` field so downstream `mass_propers` / breviary renderers see
it without re-reading the input.

**B10 prerequisite work** (lands before any breviary slice runs):

1. Add `pub psalmvar: bool` to `OfficeInput` (default `false`).
2. Add `pub votive: Option<VotiveKind>` to `OfficeInput`.
3. Add `pub cursus: Cursus` to `OfficeInput` (default `Roman`).
4. Add the same three fields to `OfficeOutput` so downstream
   renderers don't have to re-thread `OfficeInput`.
5. Define `VotiveKind` and `Cursus` enums in `src/core.rs`.
6. Update `compute_office` to honour `votive` (short-circuit) and
   `cursus` (delegate to stub for non-Roman).

This is a small, mechanical change — the precedence engine itself
doesn't move; we add a pre-check that routes to a votive resolver.
Mass-side parity is preserved because today every caller passes
`votive: None` implicitly (the field is new and defaults to `None`).

### 7.5 — GABC chant tones: preserved in data, stripped in default render

The `;;<tone>` suffix on antiphon bodies stays in `HorasFile.sections`
(it's already there). The default renderer strips it (already strips
at the proper-splice stage in `crate::horas`). A future GABC layer
can opt into the un-stripped form via a `RenderConfig::keep_chant_tones`
field. No B10 work; documented for the record.

### 7.6 — Commit cadence: coarse for the base port, fine for refinement

The Mass-side leg produced ~700 small commits driven by per-cell
regression failures. The breviary base port follows a **coarser
rhythm**: each B-slice ships as one or a small handful of commits
covering the bulk port from Perl + the pure-functional reshape. After
the base port lands, **refinement is fine-grained** — per-rubric edge
cases, per-feast oddities, per-line scrub fixes each get their own
small commit, mirroring the Mass-side regression-driven pattern.

Concretely:

| Phase | Commits per slice | Style |
|---|---|---|
| B10 — `setupstring` extraction + scaffolding | 1–3 commits | bulk port |
| B11 — concurrence | 1–2 commits | bulk port |
| B12–B14 — small slices | 1 commit each | bulk port |
| B15 — psalter | 2–4 commits (split per cursus / per hour) | bulk port |
| B16–B18 — medium slices | 2–3 commits each | bulk port |
| B19 — Matins | 4–6 commits (one per submodule) | bulk port |
| B20 — postprocess + orchestrator | 1–2 commits | bulk port |
| **B21+ — regression-driven refinement** | many small commits | per-failure |

The B21 refinement phase mirrors leg-A's ratio: ~5 % of total commit
volume during the bulk port + ~95 % during regression-driven
refinement.

A single B-slice's bulk-port commit message should:

- Reference the Perl line range it ports.
- List the new public functions added.
- Note any architectural decisions deviating from the plan above.
- **Not** include "WIP" or "rough cut" tags — the bulk port commit
  must compile, pass existing tests, and have at least one new test
  exercising the new public API.

---

## 8. Out of scope for this plan

- **Translations** beyond Latin. The Mass side ships Latin-only; the
  breviary will too. English / German / French come after parity.
- **HTML rendering.** Stays in `demo/breviary.js` and downstream
  consumers. The Rust core emits structured `RenderedLine` only.
- **Calendar (`/wip/calendar`) extensions.** Calendar layer is fed by
  the same `OfficeOutput` and needs no changes for the breviary.
- **Mass-side parity polish.** Ongoing in leg-A / leg-R; orthogonal
  to leg-B.
- **Cistercian (Altovadensis), pre-Trident Monastic, and Ordo
  Praedicatorum cursus.** The `Cursus` enum exists and the input
  struct accepts these variants; the implementations are
  `unimplemented!()` stubs in `src/breviary/monastic.rs` and
  `src/breviary/altovadum.rs`. **The API surface is stable** — adding
  the real implementations later is a code-only change, not a
  signature change. See §7.3.

---

## 9. Cross-references

- `docs/BREVIARY_PORT_SCOPE.md` — original scoping doc (LOC budgets,
  phase plan).
- `docs/SUPER_PLAN.md` — overall project tracker; B-leg row.
- `docs/UPSTREAM_WEIRDNESSES.md` — Mass-side. A breviary equivalent
  (`UPSTREAM_WEIRDNESSES_BREVIARY.md`) lands during B19 / B20.
- `docs/DIVINUM_OFFICIUM_PORT_PLAN.md` — Mass-side phase plan; B-leg
  inherits the architectural conventions documented there.
- `docs/REGRESSION_RESULTS.md` / `docs/BREVIARY_REGRESSION_RESULTS.md`
  — current parity numbers.
