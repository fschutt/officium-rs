# SUPER PLAN — full Divinum Officium replica in WASM

The end state for this repo: a **byte-for-byte replica of the
DivinumOfficium.com Perl site**, served as a fully static GitHub Pages
deploy backed by a single WebAssembly bundle. Every render must come
from a pure function over an embedded, postcard+brotli compressed
data corpus — zero hardcoded Latin in JS or Rust, zero CGI, zero
Perl at runtime.

Three legs reach that end state. **Each leg has its own loop step** —
when a wakeup ping fires the `/loop` skill, the rule below picks the
next active leg.

```
   ┌────────────────────────────────────────────────────────────┐
   │                                                            │
   │   B — Breviary port + deploy                               │
   │   C — Correctness shake-down (the 5 documented patterns)   │
   │   K — Compression / bundle-size finish-line                │
   │                                                            │
   │   Plus the cross-cutting hardcode-audit refactors R2…R5    │
   │   (these unblock C and shrink the bundle for K).           │
   │                                                            │
   └────────────────────────────────────────────────────────────┘
```

All three converge on the same demo deploy at
<https://fschutt.github.io/officium-rs/>. When B / C / K are all
green, the super-plan is done — the URL serves the same calendar +
Mass + Breviary as the upstream Perl site, in 100 % parity, in
≤ 1 MB of WASM, off a static bucket.

---

## Status board

| Leg | Phase | Status | Owner | Wakeup-cue |
|-----|-------|--------|-------|------------|
| **R** (hardcode audit refactor) | R1 — Mass Ordinary in JS → Ordo.txt walker | ✅ DONE 2026-05-04 (commit `426599d`) | — | — |
| R | R1.5 — Render-text scrub mirroring `webdia.pl` (wait[0-9]+ + extensible scrub list) | ✅ DONE 2026-05-05 — `src/scrub.rs` + `apply_render_scrubs` post-process; eliminates `wait5`/`wait10`/`wait16` leak in Mass output. Source-of-truth: `webdia.pl:651-682`. Architecture: scrub at Rust render boundary, JSON corpus stays a faithful transcode | — | — |
| R | R2 — Substring feast detection → kalendar lookup | ⏳ pending | — | when C surfaces a date this would close |
| R | R3 — Hardcoded date branches (Jan-12 etc.) → Sunday-letter table | ⏳ pending | — | C-leg unblocker |
| R | R4 — Inline-conditional grammar tables | ⏳ pending | — | C-leg unblocker |
| R | R5 — `RankKind` from numerics | ⏳ pending | — | low-priority polish |
| **B** (Breviary) | B1 — Build pipeline (psalms, horas, ordinarium → JSON) | ✅ DONE 2026-05-04 (commit `b2d227c`) — 1,204 horas keys + 202 psalms; src/horas.rs loader + 4 tests passing | — | — |
| B | B2 — Hour walker over Ordinarium template (Vespers first) | ✅ DONE 2026-05-04 (commit `b890da3`) — `compute_office_hour` walker + macro expansion; 3 new tests | — | — |
| B | B3 — Vespers (single hour) end-to-end Perl-parity smoke | ✅ DONE 2026-05-04 (commit `94b37cd`) — commune-chain resolver + per-day proper splicing | — | — |
| B | B4 — Lauds + Prime + Tertia/Sexta/Nona + Compline | ✅ DONE 2026-05-05 (commit `104630a`) | — | — |
| B | B5 — Matins (the densest hour) | ✅ DONE 2026-05-05 — Invitatorium splice + multi-Lectio emission (Lectio1..9 with intervening Responsories) via `splice_matins_lectios`; 3 new tests; Lectio4 (Monica proper) + Invitatorium antiphon both verified | — | — |
| B | B6 — Concurrence + first-vespers split | ✅ DONE 2026-05-05 — 4 slices: Te Deum (`a653808`), `[Rule] 3 lectiones` (`20c350b`), nocturn-antiphon grouping (`f58dbcd`), first-vespers concurrence helpers (`parse_horas_rank` + `first_vespers_day_key` — caller-driven rank compare so the walker stays a pure projection). 9 new tests across the 4 slices | — | — |
| B | B7 — Demo `/breviary.html` page + WASM API | ✅ DONE 2026-05-05 — Slice a (`ae21198`): `compute_office_full` WASM API. Slice b/c (this commit): `demo/breviary.html` + `demo/breviary.js` with hour selector + day_key field + first-vespers swap surfaced in UI; three-page nav (Mass / Breviary / Calendar) wired in `index.html`. Pages CI rebuilds the WASM pkg on push | — | — |
| B | B8 — Year-sweep regression to ≥ 99.7 % (all 8 hours × 5 rubrics) | 🟡 in progress 2026-05-05 — Slices 1-8 ✅. Slice 9: attempted `mass::expand_macros` on office bodies — regressed (63.33% → 46.67%) so reverted; comparator already accepts the unexpanded form via substring match. **60-day Vespera 1570 sweep: 66.67% match (40/60).** All remaining Differs are Tempora-vs-Sancti precedence gaps shared with the Mass side. Documented patterns closed + patterns reverted in `docs/BREVIARY_REGRESSION_RESULTS.md` | — | next wakeup |
| **C** (correctness) | C1 — Local span-configurable runner (`scripts/regression.sh day|year|decade|century`) | ⏳ pending | — | after B1 |
| C | C2 — Drive Sancti/01-12 cluster to 0 fail-years | 🟡 spot-checked 2026-05-05 — `Sancti/01-12` did not fire on any of 2008/2013/2019/2030/2035 in current code; the cluster appears already closed by recent precedence work. Needs full ±50yr CI rerun to confirm before marking DONE | — | run CI sweep |
| C | C3 — Drive Tempora/Pasc1-0t cluster to 0 | ⏳ pending | — | parallel with C2 |
| C | C4 — Drive Commune/C10b (Sat-BVM) cluster to 0 | ⏳ pending | — | parallel with C2 |
| C | C5 — Drive Sancti/02-23o (bissextile) cluster to 0 | ⏳ pending | — | needs `date.rs` look |
| C | C6 — Drive Sancti/05-04 cluster to 0 | ⏳ pending | — | low fail-count, late |
| **K** (compression / size) | K1 — Bundle-size budget table + per-data-file breakdown | ⏳ pending | — | after B-leg ships (Breviary corpus is 2-3× Mass) |
| K | K2 — Try shared-dictionary brotli for `missa_latin` + `horas_latin` | ⏳ pending | — | after K1 |
| K | K3 — Drop `regression` feature from default; smaller release artefact | ⏳ pending | — | small win |
| K | K4 — `wasm-opt -Oz` already wired; revisit after each leg ships | ✅ already wired in pages.yml | — | — |
| K | K5 — Final published budget: ≤ 1 MB raw / ≤ 700 KB brotli total | ⏳ pending | — | super-plan exit |
| **D** (deploy) | D1 — Calendar page (`/calendar.html`) | ⏳ pending — defer to after B7 | — | bundles with leg-B |
| D | D2 — Three-page nav (Mass / Breviary / Calendar) | ⏳ pending | — | bundles with leg-B |
| D | D3 — Per-leg CI workflow (`mass.yml`, `breviary.yml`, `calendar.yml`) | ⏳ pending | — | after C1 (uses local runner) |
| D | D4 — Cloudflare Pages mirror (optional, per user pref `master` branch) | ⏳ pending | — | only if user asks |

---

## Loop rule

When the `/loop` wakeup fires:

1. Read this file. Pick the **first row** with status `⏳ next` or `⏳ pending`
   whose dependencies are all `✅ DONE`. That's the active task.
2. Work on it for one wakeup-window (≤ 30-60 min of work; finite chunks
   only — no open-ended exploration).
3. At end of window, update the row's **Status** to `🟡 in progress
   (commit X)` if not finished, or `✅ DONE (commit X)` if shipped.
4. Commit. Push. Schedule the next wakeup.

Picking precedence when multiple rows are eligible:
- Active leg-B unless blocked. (Breviary is the longest critical path.)
- Switch to leg-C when leg-B is paused waiting on something else.
- Switch to leg-K only when both B and C are blocked or when bundle
  budget breaks (>1 MB).
- Switch to leg-R refactors only when explicitly unblocking a B/C task.

---

## Hard exit criteria (when this plan is done)

1. **Breviary parity**: ≥ 99.7 % output match against upstream Perl
   for all 8 hours × 5 rubrics × ±50 years (3.4 M cells).
2. **Mass parity**: ≥ 99.95 % across the same 100-year sweep
   (currently 99.86 %; the 5 documented patterns close it).
3. **Calendar parity**: 100 % match for `winner / commemoratio /
   color / season / rank` for all 5 rubrics × 100 years.
4. **Bundle**: ≤ 1 MB raw / ≤ 700 KB brotli for the WASM .wasm; demo
   site under 1.2 MB total payload.
5. **No hardcode**: zero hardcoded Latin in `demo/`; the 5 categories
   from `HARDCODE_AUDIT.md` (A–E) are all marked DONE; an LLM-driven
   audit confirms no per-rubric `match` arms remain that could be
   data-table lookups.
6. **Demo lives at `https://fschutt.github.io/officium-rs/`** with
   three-page nav (Mass / Breviary / Calendar), all rubrics
   selectable, all 8 hours renderable. Identical-text comparison
   against the Perl site for 12 spot-check dates passes.
7. **CI**: three green badges (mass / breviary / calendar regression)
   on the README. The local `scripts/regression.sh` runner can do
   `day | week | year | decade | century` against any rubric.

---

## Working notes feeding into the plan

- `docs/REGRESSION_RESULTS.md`: the 5 fail patterns (Sancti/01-12,
  Tempora/Pasc1-0t, Commune/C10b, Sancti/02-23o, Sancti/05-04) drive
  leg-C entirely.
- `docs/BREVIARY_PORT_SCOPE.md`: 7-stage breakdown for leg-B; ~11k
  Rust LOC budgeted, ~16 wk human-time. We compress that into a
  staged-incremental-deploy approach: ship Vespers first, expand
  outward.
- `docs/COMPRESSION_BENCH.md`: postcard+brotli decision is settled;
  leg-K is about applying the same to the Breviary corpus and
  exploring shared-dictionary tactics.
- `docs/UPSTREAM_WEIRDNESSES.md`: 36 documented anomalies; leg-C will
  add Breviary entries (as `UPSTREAM_WEIRDNESSES_BREVIARY.md`).
- `docs/HARDCODE_AUDIT.md`: phases R1 done, R2–R5 are interleaved
  with leg-C as fixes turn into refactors.

---

## Definition of "active task"

The row currently being worked. Only one across all legs at a time
(this is a single-threaded loop). When we wake up:

```
ACTIVE LEG:    B
ACTIVE TASK:   C4 — Sat-BVM (Commune/C10b) Graduale +
               Offertorium seasonal-variant resolution
ESTIMATED:     1-2 loop windows. Local spot-check found the
               concrete gap: 2008-01-26 (Saturday in
               Septuagesima) — both sides agree the day's
               winner is Commune/C10b (Sat-BVM Mariæ), but
               the Graduale section content differs.
               Perl emits "Speciósus forma præ fíliis…"
               (the Septuagesima-variant Graduale + a
               trailing Tractus). Rust emits the
               Per-Annum default Graduale.
               Root cause: `Commune/C10b` upstream has
               multiple `[Graduale]` headers annotated by
               season — `[Graduale](tempore Septuagesima)`,
               `[Graduale](tempore Quadragesima)`, etc.
               The Mass resolver currently picks the bare
               `[Graduale]` (Per-Annum default). Need to
               select the seasonal variant matching the
               date's `season`.
EXIT WHEN:     2008-01-26 Tridentine 1570 SanctaMissa
               renders the Septuagesima Sat-BVM Graduale
               byte-for-byte (Speciósus forma…). Same fix
               likely closes 2027-01-30, the other C10b
               failures across years.
```

Update this block on every wakeup so the next iteration knows what
was in flight.
