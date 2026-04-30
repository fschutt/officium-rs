# Divinum Officium → Rust port: master plan

Long-form companion to [`DIVINUM_OFFICIUM_PLAN.md`](DIVINUM_OFFICIUM_PLAN.md).
That doc covers deployment shape (static-only Cloudflare Pages, client-side
translation, URL scheme, `/wip/missal-checks` punch list). **This** doc
covers the **rubric core port** — replacing
`md2json2/src/divinum_officium/precedence.rs` (a 4-class approximation of
the 1962 rubrics, see its own header) with a faithful, pure-functional
port of the upstream Perl source, calibrated against
`divinumofficium.com`'s output day-by-day.

The current WIP pages produce wrong output because the simplified
precedence picks the wrong winner on a non-trivial fraction of days, and
every downstream layer (`missa.rs` lookup, `missal.rs` rendering) then
fetches the wrong text.

## Goal

Encode the rubrical decisions of every reform from Pius V (1570) through
John XXIII (1962) as a composable stack of pure Rust functions. Output
must match upstream `divinumofficium.com` byte-for-byte after
whitespace/punctuation normalisation, on a year sweep, for every supported
rubric.

## Non-goals

- HTML / page rendering. That lives in `md2json2/src/{calendar,missal,wip}.rs`
  and consumes the pure core as a library.
- Translation, vernacular text, the Whitaker's Words pipeline. Covered in
  the deployment plan.
- Re-implementing CGI, cookies, browser session state, or the Apache
  stack. The Perl is invoked from CLI for regression only.
- Promoting `/wip/calendar` and `/wip/missal` out of `/wip/` until the
  Rubrics 1960 layer is green on a full-year sweep.

## Why 1570 first, reforms layered on top

The Perl branches its giant `occurrence` / `precedence` subs on `$version`,
treating Trident 1570, Trident 1910, Divino Afflatu, Reduced 1955, and
Rubrics 1960 as parallel configurations. **Historically those are not
parallel** — they are a chain of diffs. Pius V (1570) is the base; every
subsequent reform is a delta promulgated by a specific Pope on a specific
date, modifying the prior set of rubrics.

Encoding the reforms as **layered functions** rather than `match` arms on
a `$version` string gives us:

1. **Self-explaining code.** `apply_pius_xii_1955(office)` reads as the
   actual rubrical change.
2. **Historical introspection.** A future "show me this Mass under each
   reform" UI is one function-stack swap.
3. **Smaller blast radius per phase.** Each reform is its own PR with
   its own test set; the 1570 baseline isn't perturbed by 1960 work.
4. **Genuine archival value.** dubia.cc is a Catholic-tradition site;
   surfacing the historical rubric chain in the rendering pipeline is
   itself part of the product.

Cost: a longer wall-clock to a working 1962 page. The WIP banner already
covers us; the existing `/wip/{calendar,missal}` keep their current output
(with banner) until Phase 11 wires the pure core in.

## Reform stack

| Layer | Year   | Promulgator     | DO `$version` string         | Phase |
|------:|--------|-----------------|------------------------------|------:|
| Base  | 1570   | Pius V          | `Tridentine - 1570`          |   3–5 |
| 1     | 1910   | (sanctoral)     | `Tridentine - 1910`          |     7 |
| 2     | 1911   | Pius X          | `Divino Afflatu - 1954`      |     8 |
| 3     | 1955   | Pius XII        | `Reduced - 1955`             |     9 |
| 4     | 1960   | John XXIII      | `Rubrics 1960 - 1960`        |    10 |
|     — | —      | —               | `Monastic`                   |    12 |

Layer 4 (Rubrics 1960) is the rubrical body underlying the typical 1962
*Missale Romanum* and *Breviarium Romanum*. Phase 10 is the gate for
promoting `/wip/calendar` and `/wip/missal` out of WIP (Phase 11).

Sources of truth for the rubric content of each reform:

- **DiPippo, *Compendium of the Reforms of the Roman Breviary,
  1568–1961*** (newliturgicalmovement.org). The chronological narrative.
- The actual Bulls / Decrees / *Rubricæ generales* per layer.
- The DO Perl source as the operational test oracle.
- For Mass: 1962 *Missale Romanum* typical edition (Baronius reprint).

Quoted from FAJ-Munich, 2025-02-17 (cited in `DIVINUM_OFFICIUM_PLAN.md`):
> "please do not try to conceive the logic of the Breviary from looking
> at the code. It has caused me nightmares. … fix the underlying rubric
> understanding first, not the code."

We treat the Perl as the **oracle for what current output is**, not the
**source of truth for what correct output is**. When Perl and DiPippo
disagree, DiPippo wins, and the regression test for that day pins our
Rust output to DiPippo (with a comment-out marker on the Perl-mismatch
case so we can audit the upstream divergence).

## Architecture

### Hard rules

1. **No globals.** Every Perl `our $foo` becomes a struct field on the
   input or output type. Sub-functions take typed inputs and return
   typed outputs. The Perl pattern of `precedence()` mutating `$winner`,
   `$commemoratio`, `%winner`, `%commemoratio`, `$rank`, `$duplex`,
   `$rule`, `$comrank`, etc. (`missa/missa.pl:36-65`) is the **first
   thing** we drop.
2. **No I/O in the core.** Sancti / Tempora / Commune / Kalendaria
   lookups go through a `Corpus` trait. The `BundledCorpus` impl wraps
   today's `data/*.json` `OnceLock`s.
3. **No regex compilation in hot paths.** Compile once at module init;
   pass `&Regex` references.
4. **Reform layers compose explicitly.** Each historical reform is
   represented as a `ReformLayer` value. The active layer chain is a
   function of `Rubric`. Layer effects fan out across the pipeline
   (kalendar diff, rubric overrides, corpus overrides).
5. **Provenance is a first-class output.** `OfficeOutput.reform_trace`
   records which reform layers fired for the day's resolution. Lets the
   future "compare under each rubric" UI work, and lets the regression
   harness explain *why* Rust diverges from Perl when it does.

### Type sketch

```rust
// md2json2/src/divinum_officium/core.rs (new)

pub enum Rubric {
    Tridentine1570,
    Tridentine1910,
    DivinoAfflatu1911,
    Reduced1955,
    Rubrics1960,
    Monastic,
}

pub struct OfficeInput {
    pub date: NaiveDate,    // Gregorian
    pub rubric: Rubric,
    pub locale: Locale,     // Latin for the pure core; vernacular is
                            // assembled downstream by translation layer
}

pub struct OfficeOutput {
    // ── Mirrors what Perl precedence() writes to globals ────────────
    pub winner: FileKey,                  // "Sancti/04-29", "Tempora/Pasc3-0"
    pub commemoratio: Option<FileKey>,
    pub scriptura: Option<FileKey>,       // when winner is sanctoral
    pub commune: Option<FileKey>,
    pub commune_type: CommuneType,        // ex | vide | none
    pub rank: Rank,                       // class + duplex + rank_num
    pub rule: Vec<RuleLine>,              // [Rank] body, parsed
    pub day_kind: DayKind,
    pub season: Season,
    pub color: Color,
    pub vespers_split: Option<VespersSplit>,    // Office only; Mass = None

    // ── Provenance ──────────────────────────────────────────────────
    pub reform_trace: Vec<ReformAction>,        // which layers did what
}

pub struct MassPropers {
    pub introitus:    Option<ProperBlock>,
    pub oratio:       Option<ProperBlock>,
    pub lectio:       Option<ProperBlock>,
    pub graduale:     Option<ProperBlock>,
    pub tractus:      Option<ProperBlock>,
    pub evangelium:   Option<ProperBlock>,
    pub offertorium:  Option<ProperBlock>,
    pub secreta:      Option<ProperBlock>,
    pub communio:     Option<ProperBlock>,
    pub postcommunio: Option<ProperBlock>,
    // …commemorations, sequence, prefatio, etc.
}

pub struct ProperBlock {
    pub latin: String,
    pub source: FileKey,    // where in the corpus this body came from
    pub via_commune: bool,  // true = pulled via @Commune fallback
}

pub trait Corpus {
    fn sancti(&self, key: &FileKey) -> Option<&MassFile>;
    fn tempora(&self, key: &FileKey) -> Option<&MassFile>;
    fn commune(&self, key: &FileKey) -> Option<&MassFile>;
    fn kalendaria(&self, year: i32, rubric: Rubric) -> &Kalendaria;
}

pub fn compute_office(input: &OfficeInput, corpus: &dyn Corpus) -> OfficeOutput;
pub fn mass_propers(office: &OfficeOutput, corpus: &dyn Corpus) -> MassPropers;
```

### Reform-layer composition

```rust
// md2json2/src/divinum_officium/reform.rs (new)

pub struct ReformLayer {
    pub name: &'static str,        // "Pius V 1570", "John XXIII 1960", …
    pub year: i32,
    pub kalendar_diff: KalendarDiff,
    pub rubric_overrides: RubricOverrides,
    pub corpus_overrides: CorpusOverrides,
}

pub fn reform_chain(rubric: Rubric) -> &'static [&'static ReformLayer] {
    match rubric {
        Rubric::Tridentine1570    => &[&PIUS_V_1570],
        Rubric::Tridentine1910    => &[&PIUS_V_1570, &TRIDENT_1910],
        Rubric::DivinoAfflatu1911 => &[&PIUS_V_1570, &TRIDENT_1910, &PIUS_X_1911],
        Rubric::Reduced1955       => &[&PIUS_V_1570, &TRIDENT_1910, &PIUS_X_1911, &PIUS_XII_1955],
        Rubric::Rubrics1960       => &[&PIUS_V_1570, &TRIDENT_1910, &PIUS_X_1911, &PIUS_XII_1955, &JOHN_XXIII_1960],
        Rubric::Monastic          => &[&MONASTIC],
    }
}
```

The pipeline functions consult the chain at each stage. For example:

```rust
fn occurrence(input: &OfficeInput, corpus: &dyn Corpus) -> OccurrenceResult {
    let chain = reform_chain(input.rubric);
    let mut k = corpus.kalendaria(input.date.year(), input.rubric).clone();
    for layer in chain {
        k = layer.kalendar_diff.apply(k);   // suppress / demote / move
    }
    let mut occ = base_occurrence_1570(input, &k, corpus);
    for layer in chain {
        occ = layer.rubric_overrides.apply(occ, input);
    }
    occ
}
```

The base `*_1570` functions don't know about reforms. Each
`ReformOverrides::apply` is a small focused diff (e.g.
`PIUS_XII_1955.kalendar_diff` strips most octaves; the corresponding
`rubric_overrides` removes the matching octave-rank cases).

### Vendoring the Perl reference

The Perl tree is the regression oracle. Pin: upstream commit
**`b0c1c717143b4b092c6861fe4c33b97092a852f8`** (2026-04-30, "Merge
pull request #5153 from tjrandall/hotfix/fix-plack-405-error").

We initially considered pinning to `c1776c8f89` (2025-02-21) — the
calibration anchor of the partial port at
`md2json2/src/divinum_officium/mod.rs:1-13` — but a path-grouped
`git log c1776c8f89..b0c1c71714` showed the 26 intervening commits
fall into three buckets:

- **Server-shim Perl edits** (4 files, all peripheral): `webdia.pl`
  warning fix, `officium.pl` Plack shim, `specmatins.pl`,
  `RunTimeOptions.pm`. `horascommon.pl` and the `missa/*.pl`
  resolver chain are **untouched**.
- **Corpus edits** (6 files, all out-of-scope for the Roman 1962
  path): Cistercian (`SanctiCist/…`, `Kalendaria/C1951.txt`,
  `Kalendaria/CAV.txt`) and Dominican (`SanctiOP/04-30.txt`).
- **Docker / `app.psgi` / runtime config** — not part of the
  oracle invocation.

The standard `Sancti/`, `Tempora/`, `Commune/`,
`Kalendaria/1955.txt`, and `Kalendaria/1960.txt` did not change in
that range. Bumping the pin to HEAD is therefore zero-risk for the
regression target *and* picks up the Plack pin fix that the local
Docker fallback may need. The pin file lives at
`scripts/divinum-officium.pin`.

- Path: `vendor/divinum-officium/` (new). Added to `.gitignore`.
- Setup script: `scripts/setup-divinum-officium.sh`. Idempotent.
  - If `vendor/divinum-officium/` missing: `git clone
    https://github.com/DivinumOfficium/divinum-officium vendor/divinum-officium`
  - If present: `git fetch && git checkout <pin> && git reset --hard <pin>`
    on a clean tree; refuse on a dirty tree (so a developer hand-editing
    the Perl for debugging doesn't get clobbered).
  - Print a one-line pin status at the end: "vendor pinned at c1776c8f89".
- Pin file: `scripts/divinum-officium.pin` — single line, the SHA. Setup
  script reads it; bumping the pin is a one-line PR with a regression
  rerun.
- Test guard: every regression test starts with `assert_vendor_present()`
  which checks `vendor/divinum-officium/web/cgi-bin/missa/missa.pl`
  exists. On miss, prints the literal `bash scripts/setup-divinum-officium.sh`
  invocation.
- CI: a setup step runs the script; cache `vendor/divinum-officium/`
  keyed on the pin file's SHA. The cache key flips when the pin moves;
  forces a full regression rerun on bump.
- Why not a submodule: submodule maintenance overhead (status pollution,
  `clone --recursive` gotchas, force-push hazards on upstream `master`)
  isn't worth it. A script + pinned SHA is simpler and CI-cacheable.

### Perl invocation shape

The DO regression script (`vendor/divinum-officium/regress/scripts/generate-diff.sh:55-67`)
already documents the CGI-as-script invocation:

```sh
perl vendor/divinum-officium/web/cgi-bin/missa/missa.pl \
  "version=Tridentine - 1570" \
  "command=praySanctaMissa" \
  "date=04-30-2026" | \
grep -Pv '^Set-Cookie:'
```

We wrap that in `scripts/do_render.sh DATE VERSION HOUR`. `HOUR ∈
{SanctaMissa, Matutinum, Laudes, Prima, Tertia, Sexta, Nona, Vespera,
Completorium}`. For Mass, only `SanctaMissa`. CPAN deps from
`vendor/divinum-officium/Build.pl`: CGI, CGI::Cookie, DateTime,
List::MoreUtils, Time::Local, Test::Cmd, Test::Carp. Setup script
verifies them with `perl -e "use CGI; use DateTime; ..."` and prints
the missing-module error verbatim, with a `cpanm` install hint.

If local CPAN install fights us → fall back to
`docker compose -f vendor/divinum-officium/docker-compose.yml up -d`
and `docker exec` into the container. Deferred unless we hit the
fallback path.

## File layout (target)

```
md2json2/src/divinum_officium/
├── mod.rs                # public surface
├── core.rs               # types: OfficeInput, OfficeOutput, MassPropers, …
├── corpus.rs             # Corpus trait + BundledCorpus impl
├── date.rs               # KEPT — already pure-functional; minor touch-ups
├── reform.rs             # ReformLayer, reform_chain(), per-layer constants
├── reform_1570.rs        # PIUS_V_1570 baseline data
├── reform_1910.rs        # TRIDENT_1910 deltas
├── reform_da_1911.rs     # PIUS_X_1911 (Divino Afflatu) deltas
├── reform_1955.rs        # PIUS_XII_1955 deltas
├── reform_1960.rs        # JOHN_XXIII_1960 deltas
├── kalendaria.rs         # KEPT, refactored to be reform-aware
├── sancti.rs             # KEPT
├── occurrence.rs         # NEW — port of horascommon.pl:20-697
├── precedence.rs         # REPLACED — port of horascommon.pl:1375-1675
├── concurrence.rs        # NEW (deferred — Office only)
├── mass.rs               # NEW — port of missa/propers.pl resolver subs
└── translation.rs        # KEPT (Phase 1 lemma layer; deployment plan)
```

```
md2json2/tests/
├── unit/                 # baked-in hand-curated dates per phase
│   ├── occurrence_1570.rs
│   ├── precedence_1570.rs
│   ├── reform_1955.rs
│   └── …
└── regression/           # feature-flagged; shells out to Perl
    ├── harness.rs
    ├── extractor.rs      # parse Perl HTML → section bodies
    ├── year_sweep.rs
    └── reports/          # JSON output: target/regression/*.json
```

```
scripts/
├── setup-divinum-officium.sh    # clone or pin
├── do_render.sh                 # wrap perl missa.pl
└── divinum-officium.pin         # SHA file
```

## Phase plan

Each phase is one PR (or a small stack). Each phase ends with: tests
green, regression report committed, plan-doc status block updated.

### Phase 0 — Vendoring + Perl CLI harness

**Deliverables.**
- `vendor/divinum-officium/` added to `.gitignore`.
- `scripts/setup-divinum-officium.sh` clones / pins.
- `scripts/divinum-officium.pin` set to `c1776c8f89ca23dd560f755d632488cee9496957`.
- `scripts/do_render.sh DATE VERSION HOUR` wraps `perl missa.pl …`.
- README block in `DIVINUM_OFFICIUM_PORT_PLAN.md` (this file)
  describing setup. CI step pulled.

**Acceptance.** `bash scripts/do_render.sh 04-30-2026 'Tridentine - 1570'
SanctaMissa | grep -i Introitus` exits 0 against a freshly-vendored
tree.

### Phase 1 — Pure-core types + Corpus trait

**Deliverables.**
- `md2json2/src/divinum_officium/core.rs` with `OfficeInput`,
  `OfficeOutput`, `MassPropers`, `ProperBlock`, `Rubric`, `Rank`,
  `DayKind`, `Season`, `Color`, `FileKey`, `CommuneType`, `RuleLine`,
  `ReformAction`, `Locale`.
- `md2json2/src/divinum_officium/corpus.rs` with `trait Corpus` +
  `struct BundledCorpus` wrapping today's `data/*.json` `OnceLock`s.
  `MassFile`, `SanctiEntry`, `KalendariaEntry` move under `Corpus`.
- `md2json2/src/divinum_officium/reform.rs` skeleton: `struct
  ReformLayer`, `fn reform_chain(rubric)`, empty `KalendarDiff` /
  `RubricOverrides` / `CorpusOverrides` types.

**Acceptance.** `cargo check` passes. No logic yet. Existing
`precedence::decide()` still wired into `calendar.rs` / `missal.rs`;
nothing breaks.

### Phase 2 — Date math verified against Perl for all rubrics

**Deliverables.**
- `date.rs` ported sub `getweek` audited against
  `horascommon.pl::gettoday` + `Date.pm::getweek`. Differences
  reconciled.
- New tests: ~30 dates per rubric, asserting `getweek(date, rubric) ==
  perl_getweek(date, rubric)`. Includes Easter ±35 days, Septuagesima,
  Advent boundaries, Christmas Octave, days that cross
  Tridentine/post-Tridentine kalendar boundaries.
- A small CLI binary `cargo run --bin getweek-check -- --year 2026
  --rubric Rubrics1960` that loops a year and diffs against Perl.

**Acceptance.** Year sweep for 2026 against `Tridentine - 1570`
emits 0 mismatches.

### Phase 3 — `occurrence()` port, Tridentine 1570 only

**Deliverables.**
- `md2json2/src/divinum_officium/occurrence.rs`. 1:1 port of
  `vendor/divinum-officium/web/cgi-bin/horas/horascommon.pl:20-697`,
  but with all `if ($version =~ /1960/) … elsif (/1955/) …` branches
  *deleted* — only the 1570 path. Each deleted branch leaves a
  marker comment: `// reform-PIUS-XII-1955: see Phase 9`.
- Hand-curated unit tests in `md2json2/tests/unit/occurrence_1570.rs`,
  ~25 dates spanning the fault lines:
  - Sundays of post-Pentecost meeting Class III sanctoral
  - Vigils of Christmas, Pentecost, Ascension
  - 17–24 Dec privileged ferias
  - Holy Week (esp. Maundy Thursday → Holy Saturday)
  - Octave days: Easter, Pentecost, Christmas, Epiphany
  - Greater ferias of Lent
  - All-Souls' Day
  - Suppressed-by-1955 dates *as the 1570 calendar still has them*
- Each test cites its rubrical source (DiPippo page, *Rubricæ
  generales* §, or the Perl line).

**Acceptance.** Unit suite green. No regression harness yet.

### Phase 4 — `precedence()` port, Tridentine 1570 only

**Deliverables.**
- `md2json2/src/divinum_officium/precedence.rs` rewritten. 1:1 port
  of `horascommon.pl:1375-1675` for the 1570 path only. Deletes the
  Class I/II/III/IV approximation; uses `Rank` from `core.rs`.
- The simplified `decide()` API removed. Existing callsites in
  `calendar.rs` / `missal.rs` temporarily wired to a stub that
  returns the existing approximation, so the WIP pages keep
  rendering. A `// TODO Phase 11` comment marks the stub.
- Hand-curated unit tests in `md2json2/tests/unit/precedence_1570.rs`,
  ~25 dates.

**Acceptance.** Unit suite green. WIP pages still render (with stub).

### Phase 5 — Mass-propers resolver, Tridentine 1570 only

**Deliverables.**
- `md2json2/src/divinum_officium/mass.rs`. Ports from
  `vendor/divinum-officium/web/cgi-bin/missa/propers.pl`:
  - `getproprium` → `proper_block(office, section, corpus) -> Option<ProperBlock>`
  - `getfromcommune` → `commune_block(office, section, corpus) -> Option<ProperBlock>`
  - the `setbuild()` chain → `mass_propers(office, corpus) -> MassPropers`
  - `oratio()` → `oratio_block(office, corpus, ord) -> Option<ProperBlock>`
  - the `@Commune/Cxx-y` chain follower (already half-handled in
    today's `missa.rs:60-77`) — fully implement, including the
    `:Lectio7 in N loco` substitution.
- Hand-curated unit tests, ~20 dates × all Mass sections, asserting
  exact Latin-text equality against the typical edition (Baronius
  reprint of the *Missale Romanum* 1570 reissue, since most of these
  haven't drifted).

**Acceptance.** Unit suite green. WIP pages still render via stub
from Phase 4.

### Phase 6 — Regression harness Rust ↔ Perl, 1570 year sweep

**Deliverables.**
- `md2json2/tests/regression/harness.rs` (feature-flagged
  `regression`). For each `(date, Rubric::Tridentine1570)`:
  1. Rust: `compute_office` → `mass_propers`.
  2. Perl: `do_render.sh DATE 'Tridentine - 1570' SanctaMissa` → HTML.
  3. Extract per-section bodies (`extractor.rs`: regex-or-html-parse
     against the stable `<H2>Introitus</H2>` shape).
  4. Substring assertion per section: each Latin block from Rust
     output must appear inside the corresponding Perl block, modulo
     whitespace + punctuation normalisation.
- `cargo run --bin year-sweep -- --year 2026 --rubric Tridentine1570`
  emits `target/regression/Tridentine1570-2026.json` (per-day,
  per-section pass/fail) and `.html` (the green/yellow/red board).
- Failures bucket by:
  - `winner` mismatch — precedence picked the wrong file
  - `commune` mismatch — winner OK but Common fallback wrong
  - body mismatch — resolver inserted wrong section body
  - transcription drift — whitespace / punctuation only (auto-pass)

**Acceptance.** Year sweep on 2026 / Tridentine 1570 has ≥99% green
cells. Remaining red/yellow cells get pinned unit tests in Phases 3–5.

### Phase 7 — Reform layer Tridentine 1910

**Deliverables.**
- `reform_1910.rs`: the 340-year diff between Pius V's 1570 sanctoral
  and the kalendar as it stood in 1910 (added saints, raised feasts,
  octave additions). Mostly kalendar diff, almost no rubric override.
- Unit tests for ~10 dates that gain a feast in 1910 vs. 1570.
- Year sweep for 2026 / Tridentine 1910 added to the regression
  harness output. Both 1570 and 1910 boards visible.

**Acceptance.** Year sweep on 2026 / Tridentine 1910 ≥99% green.

### Phase 8 — Reform layer Divino Afflatu (Pius X, 1911)

**Deliverables.**
- `reform_da_1911.rs`: psalter rearrangement (Office only — Mass
  unaffected for the most part), suppression of certain octaves and
  sanctoral demotion. Mass changes mostly cosmetic but documented.
- Unit tests + year sweep for 2026 / Divino Afflatu.

**Acceptance.** Year sweep ≥99% green.

### Phase 9 — Reform layer Reduced 1955 (Pius XII)

**Deliverables.**
- `reform_1955.rs`: the 1955 *Cum nostra* general rubrics simplification
  — most octaves stripped (only Christmas / Easter / Pentecost left),
  Holy Week reform, vigils trimmed, sanctoral demotions. **Major
  diff** — this is the biggest rubric layer in volume.
- Holy Week reform deserves its own sub-test set: Palm Sunday,
  Maundy Thursday, Good Friday, Holy Saturday — each with its own
  pinned propers.
- Year sweep for 2026 / Reduced 1955.

**Acceptance.** Year sweep ≥99% green.

### Phase 10 — Reform layer Rubrics 1960 (John XXIII)

**Deliverables.**
- `reform_1960.rs`: the *Rubricæ generales Breviarii et Missalis
  Romani* (1960). Rank-class consolidation (I/II/III/IV), sanctoral
  cuts, commemoration cuts, lection structure changes.
- Year sweep for 2026 / Rubrics 1960.

**Acceptance.** Year sweep on 2026 / Rubrics 1960 ≥99% green. **This
is the gate** for Phase 11 (un-WIP-ing the pages).

### Phase 11 — Wire pure core into `/wip/calendar` + `/wip/missal`

**Deliverables.**
- `md2json2/src/calendar.rs:24-26` and `md2json2/src/missal.rs:18-20`
  switch from the `precedence::decide()` stub to:
  ```rust
  let office = compute_office(&OfficeInput {
      date,
      rubric: Rubric::Rubrics1960,
      locale: Locale::Latin,
  }, &CORPUS);
  let propers = mass_propers(&office, &CORPUS);
  ```
- The simplified-stub holdover from Phase 4 is deleted.
- WIP banner kept until the regression report has shown ≥99% green
  on 2026 + 2027 + 2028 (three consecutive year sweeps clean).
- `/wip/missal-checks` page (described in `DIVINUM_OFFICIUM_PLAN.md:399-405`)
  becomes the public-facing version of the regression board.

**Acceptance.** `/wip/calendar` and `/wip/missal` render today's day
correctly for the calibration set in `DIVINUM_OFFICIUM_PLAN.md:96-99`
(major Sundays, Christmas, Easter, Pentecost) — verified by hand
against the Baronius typical edition.

### Phase 12 — Monastic + Tridentine 1570 / 1910 surfaces

**Deliverables.**
- `reform_monastic.rs`: separate ordo entirely. Probably best as a
  parallel chain rather than another layer on top — the `match` arm in
  `reform_chain` already handles this.
- `?rubric=…` query string (or in-page toggle) on `/wip/calendar` and
  `/wip/missal` lets the user choose any of the six rubric sets.
- The "compare under each rubric" UI proposed at the top of this doc
  ships as a fold-out on the date permalink.

**Acceptance.** Year sweeps for all six rubric sets ≥99% green.

## Test strategy

Three rings, each tighter than the last.

### Ring 1 — Unit

`#[cfg(test)] mod tests` inside each ported sub. Hand-curated
fault-line dates per rubric layer. Each phase adds ≥25 dates per layer
it touches. Tests precede the port (write the assertion, watch it
fail, port the code, watch it pass).

Each test cites its rubrical source. The DiPippo compendium and the
*Rubricæ generales* are the primary citations; the Perl line is a
secondary check.

### Ring 2 — Regression spot-check

`cargo test --features regression`. Picks 50 random `(date, rubric)`
pairs from a deterministic seed, asserts Rust output ⊆ Perl output
after normalisation. Catches drift between full-year sweeps. Runs in CI
on every push (after the vendor cache step).

### Ring 3 — Year sweep

`cargo run --bin year-sweep -- --year YYYY --rubric Rubric`. Iterates
366 days × ~10 Mass sections, emits JSON + HTML reports. Run nightly
in CI for the years 2025–2027 across all six rubrics. The HTML reports
are what `/wip/missal-checks` shows publicly.

**Triage protocol.** Each red cell on the year-sweep board becomes
a debugging task. Fix flow:

1. Open the cell — see the diff (Rust output vs Perl HTML excerpt).
2. Determine which layer's logic is wrong: occurrence (winner),
   precedence (rank), corpus lookup (commune), or resolver (body).
3. Find the Perl line that handles the case Rust missed (`grep -n` in
   the relevant `.pl` file).
4. Decide: is the Perl correct? If yes, port the missing case +
   pin a unit test. If no (DiPippo says Perl is wrong), pin the unit
   test against DiPippo + add an `// upstream divergence` marker.
5. Re-run the year sweep; cell flips green.

**Yellow cells** (Rust output appears within Perl output, but Perl
emits extra) are usually OK — Perl injects rubrical comments,
proper-text bracketing, sometimes scriptura indices. Each yellow
pattern gets a one-time triage; once classified, the extractor
strips that variety on subsequent runs.

## Open questions

1. **Date type.** `chrono::NaiveDate` (Gregorian) is easy. Pre-1582
   Gregorian dates are anachronistic for some countries (England
   adopted in 1752), but the Tridentine 1570 typical edition uses
   the Gregorian reform from 1582 onward; pre-1582 we just don't
   render. Confirm this is acceptable scope.
2. **`Tabulae/` directory.** The Perl `web/www/Tabulae/` mixes data
   (kalendar diffs the existing `kalendaria_1962.json` already
   ingests) with rendering helpers (HTML templates). Per-file audit
   needed during Phase 5; some files probably need a Rust counterpart
   in `corpus.rs`.
3. **`RuleLine` parsing.** `[Rank]` body lines like `"no Gloria;
   Credo; Preface of the BVM"` carry per-day rubric switches. Start
   as opaque strings; parse lazily as the regression harness exposes
   which switches actually matter. (At least: Gloria on/off, Credo
   on/off, which Preface, which Communicantes, which Hanc Igitur,
   sequence on/off, last Gospel on/off.)
4. **CI runtime.** A full year × six rubrics × ~10 sections is ~22 000
   regression-test cells per nightly run. Perl execution time
   dominates (~1 s per Mass render). Parallelise across cores; cap at
   60 min wall-clock or shard across jobs.
5. **Pinning DiPippo over Perl.** When we deliberately diverge from
   the Perl because DiPippo says it's wrong, do we file a bug
   upstream (good citizenship, helps everyone) or just document the
   divergence in `// upstream divergence` markers (less work)? Probably
   both — a tracking issue list at the bottom of this doc.

## Status tracking

This is a long journey. Each completed phase appends a one-line
entry below with date, commit, and headline number (e.g. "%
green on the year sweep").

| Phase | Status      | Date       | Commit  | Notes |
|------:|-------------|------------|---------|-------|
|   0   | complete    | 2026-04-30 | `1fb4ebd` | Vendor pinned at `b0c1c71714`. `scripts/{setup-divinum-officium,do_render,rebuild}.sh`. `cargo run --bin year-sweep -- --smoke` returns 3/3. All five standard rubrics render Mass HTML for 04-30-2026 in ~100 ms/date. One pre-existing sancti test marked `#[ignore = "Phase 2 corpus audit"]`. |
|   1   | complete    | 2026-04-30 | `e2aaecc` | `core.rs` (~370 LOC), `corpus.rs` (~80 LOC), `reform.rs` (~180 LOC). Types: `OfficeInput`, `OfficeOutput`, `MassPropers`, `ProperBlock`, `MassCommemoration`, `Rubric`, `Locale`, `Rank`, `RankClass`, `RankKind`, `DayKind`, `Season`, `Color`, `FileKey`, `FileCategory`, `CommuneType`, `RuleLine`, `ReformAction`, `ReformActionKind`, `VespersSplit`, `VespersSplitPoint`, `Date`. `trait Corpus` + `BundledCorpus` (bodies `todo!()` until Phase 4). `ReformLayer` + chain constants for 1570 → 1960 + Monastic. 12 new unit tests; total now 30 pass / 1 ignored. |
|   2   | complete    | 2026-04-30 | `c2e0f91` | Two `getweek` bugs found and fixed: (1) Jan-1-9 was emitting `Nat1..Nat9` unpadded — the upstream files are `Nat02.txt` etc., need `Nat{:02}`; (2) `getadvent` dropped Perl's `dow \|\| 7` truthy-or trick, shifting Advent by 7 days in any year where Christmas falls on a Sunday (e.g. 2022). New `scripts/perl_getweek_year.pl` Perl-side oracle; new `cargo run --bin getweek-check` Rust↔Perl diff binary. After fixes, **0 divergences across 1900-2100 × {missa, tomorrow}² = 4 flag combos** — ~293 600 cell checks. 11 hand-curated unit tests pin Easter, Eastertide, pre-Lent / Lent, Advent, Christmas Octave (unpadded Dec, padded Jan), Pent24-cap vs PentEpi-overflow on the `missa` flag, Christmas-on-Sunday Advent shift (2022 regression pin). `src/lib.rs` added to expose `divinum_officium` to `src/bin/*` binaries. `rebuild.sh` extended to step `[5/5]` running getweek-check for the current year. AI-port noise (10 unused-mut / unused-var warnings) cleaned. |
|   3   | complete    | 2026-05-01 | `34a7cfd` | **MVP-skeleton port, not 1:1.** New `occurrence.rs` (~340 LOC) — `compute_occurrence(input, corpus) -> OccurrenceResult` for Tridentine 1570 only. Handles: Tempora-vs-Sancti file lookup (via Corpus trait), basic numeric rank comparison, Class I-temporal-wins-solo (Easter / Pentecost / Christmas), Sunday-vs-Class-I-sanctoral, privileged-feria yields, and `commune` parsing for `vide CXX` / `ex CXX` forms. **Deferred to Phases 6-10** (with marker comments throughout): directorium-driven transfers, transferred vigils, octave bookkeeping, Saturday BVM substitution, 17-24 Dec privileged-feria table, 1570 kalendar diff (`Tabulae/Kalendaria/1570.txt`), the All-Saints-Octave-vs-All-Souls collision, "Festum Domini" exceptions. `BundledCorpus` partly wired (sancti / mass_file live; kalendaria still `NoOverride` until Phase 7). `sancti.rs` gets `raw_entries()` + `pick_by_rubric()` accessors. New `scripts/do_query.sh` extracts winner / commemoratio / scriptura headlines from `do_render.sh` HTML — Phase 3 oracle. 14 unit tests pass / 5 ignored with phase markers (St Peter Martyr 04-29, All Souls 11-02, Saturday BVM, 17-24 Dec ferias, 1570 kalendar). Total now 54 pass / 6 ignored. Other rubrics (Tridentine 1910 → Rubrics 1960) `panic!()` until their reform layers land in Phases 7-10. Note: Phase 3's plan-doc deliverable said "1:1 port of horascommon.pl:20-697"; the 678-line Perl is too entangled with file-system / regex / globals to port that literally in one shot. Phase 6 year-sweep will surface specific failures and feed the gap-fill into Phases 7-10 as those reforms land. |
|   4   | complete    | 2026-05-01 | `4f83844` | New `precedence.rs` (~390 LOC) with `compute_office(input, corpus) -> OfficeOutput` — Phase 4 orchestrator that wraps `occurrence::compute_occurrence` and produces the canonical `core::OfficeOutput`. Resolves typed `Rank` (class + kind + raw label + rank_num), derives `DayKind` (Sunday / Feria / Feast / OctaveDay / Vigil / EmberDay / RogationDay) from the winner's `[Officium]`, derives `Season` from the `getweek` label (Adv / Christmas / Septuagesima / Lent / Passiontide / Easter / PostPentecost / PostEpiphany), resolves `Color` via the existing `sancti::liturgical_color` heuristic, parses `[Rule]` lines into opaque `RuleLine` strings (Phase 5 will parse selected directives). Old simplified 4-class precedence renamed `precedence_legacy.rs` (still wired into `/wip/calendar` and `/wip/missal` until Phase 11). 19 new unit tests covering Easter / Christmas / Lent Sunday / Palm Sunday / Advent 1 / St Stephen / Trinity Sunday / Vigil detection / Rank-kind classification / Cross-check vs occurrence direct call. Total now 73 pass / 9 ignored. Rubrics other than Tridentine1570 panic with phase pointer until reform layers land. |
|   5   | complete    | 2026-05-01 | (next)    | New `mass.rs` (~270 LOC) with `mass_propers(office, corpus) -> MassPropers` — pure string-assembly resolver, no HTML, no globals. Per-section lookup with `@Path` and `@Path:Section` chain following (max 4 hops); commune fallback when winner section is empty and `commune_type ∈ {Ex, Vide}`; multi-Mass redirect for body-less meta files (Sancti/12-25 → Sancti/12-25m1). Deferred: `Section in N loco` indexed substitution and `::s/PAT/REPL/` regex substitution forms — Phase 6 year-sweep will surface concrete cases. 14 unit tests pass / 3 ignored covering Christmas / Easter / Pentecost Introitus textual anchors, Peter & Paul `@Commune/C4b` chain resolution, single-Mass-vs-meta source distinction, missing-section None return, commemorations vec empty (Phase 6 work). Total suite now 87 pass / 12 ignored. **Build-script bug fixed alongside**: `data/build_missa_json.py` `SECTION_RE` was anchored `^\\[name\\]\\s*$` which dropped every section with a trailing `(rubrica xyz)` annotation — most of `Commune/C4b` and several others. Relaxed to `^\\[name\\]` and switched to first-occurrence-wins so rubric-conditional variants don't concatenate. `data/missa_latin.json` regenerated: Commune entries grew from 1-section stubs to full propers (e.g. C4b 1 → 21 sections, total keys 1032 → 1041). |
|   6   | not started |            |        |       |
|   7   | not started |            |        |       |
|   8   | not started |            |        |       |
|   9   | not started |            |        |       |
|  10   | not started |            |        |       |
|  11   | not started |            |        |       |
|  12   | not started |            |        |       |

## Upstream-divergence tracker

When our port deliberately deviates from the Perl because the
authoritative rubric source (DiPippo + the actual Bull / *Rubricæ
generales*) says the Perl is wrong, log it here. Cross-link to the
unit test that pins our chosen behaviour.

| Date / case | Layer | Perl says | We say | Source | Test |
|-------------|-------|-----------|--------|--------|------|
| —           | —     | —         | —      | —      | —    |

(Empty until Phase 6.)
