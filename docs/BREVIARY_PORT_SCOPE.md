# Breviary (Office hours) port — scope & estimate

Scope study for porting the upstream Divinum Officium **Breviary** —
Matins, Lauds, Prime, Terce, Sext, None, Vespers, Compline — from
Perl to Rust on top of the existing Mass-only `officium-rs` crate.

Baseline references:

- Perl entry point: `vendor/divinum-officium/web/cgi-bin/horas/officium.pl`
  (285 LOC) plus 9 sibling helpers totalling **~10,572 LOC** under
  `cgi-bin/horas/`, vs. the Mass entry stack at ~2,843 LOC under
  `cgi-bin/missa/`. Breviary is roughly **3.7×** the Mass-side Perl.
- Latin corpus: `vendor/divinum-officium/web/www/horas/Latin/`
  (4,730 files, 22 MB) vs. Mass at 5.1 MB. Breviary corpus is
  **~4.3×** the Mass corpus by bytes.
- Existing Rust port: `/Users/fschutt/Development/officium-rs/src/`,
  ~12,300 LOC; `mass.rs` alone is 4,109 LOC, `regression.rs` 1,946 LOC.

---

## 1. What is reused vs. net-new

### Reused **unchanged** (already 100 % parity, all five rubric layers)

| File | What it gives the Breviary |
|---|---|
| `src/date.rs` (996 LOC) | `getweek`, leap-shift, `easter`, weekday helpers — identical needs. |
| `src/occurrence.rs` (1,908 LOC) | Tempora vs. Sancti winner selection. Breviary calls the **same** `occurrence()` Perl function (see `horascommon.pl:20-808`); only difference is the Mass-only `$missa` flag turns off. |
| `src/precedence.rs` (514 LOC) | Same orchestrator. Already documents the `vespers_split: None` slot to be filled. |
| `src/sancti.rs`, `src/transfer_table.rs`, `src/tempora_table.rs`, `src/kalendaria*.rs`, `src/kalendarium_1570.rs`, `src/reform.rs` | Calendar/rubric layers — entirely shared. |
| `src/prayers.rs` (243 LOC) | `&Pater_noster`, `&Gloria`, `&Dominus_vobiscum` macros — same prayer corpus, expanded for new tokens like `&Te Deum`, `&Capitulum`, `&mLitany`. |
| `src/data_types.rs`, `src/core.rs` (`Rubric`, `Season`, `DayKind`, `OfficeOutput`) | Enums and types are layer-agnostic. `OfficeOutput.vespers_split` field is already reserved. |
| `data/build_sancti_json.py`, `data/sancti.json`, `data/transfer_combined.txt`, `data/kalendarium_1570.txt`, `data/tempora_redirects.txt` | Calendar inputs — no change. |

### Reused **with extension**

| File | Extension needed |
|---|---|
| `src/precedence.rs` | Add `concurrence()` branch (Perl `horascommon.pl:810-1426`) producing `vespers_split: Some(VespersSplit { winner, commemoratio, capitulo_break, … })`. Drives 1st-Vespers/2nd-Vespers/a-capitulo selection. ~600 LOC of new logic — direct port of the Perl block. |
| `src/missa.rs` | Becomes a thin alias for the corpus loader. Generalize to a `breviary_file(key)` that reads from a parallel `horas_latin.postcard` blob. ~50 LOC tweak. |
| `src/data_types.rs` | New variants on `OfficeFile` (or split into `MassFile` / `OfficeFile`): the Breviary needs ~50 distinct section keys that don't appear in Mass (`Ant 1`-`Ant 9`, `Ant Matutinum`, `Ant Laudes`, `Ant Vespera`, `Ant Vespera 3`, `Hymnus Matutinum`/`Laudes`/`Vespera`/`Tertia`/`Sexta`/`Nona`/`Completorium`, `Capitulum Laudes`/`Vespera`/`Tertia`/…, `Responsory*`, `Versum*`, `Lectio1`–`Lectio9`, `Responsory1`–`Responsory9`, `Invit`, `Oratio Vespera`, `Oratio Matutinum`, `Special $hora`, `Prelude $hora`, `Ant Completorium$vespera`). |
| `src/regression.rs` | Add per-hour cell extraction (currently Mass-only). Year-sweep grows from 21,900 cells × 5 rubrics to ~60,000 cells × 5 rubrics × 8 hours. |
| `src/translation.rs` | Currently English; identical pipeline applies. |

### Net-new modules

| Path | Purpose | Est. LOC |
|---|---|---|
| `src/horas.rs` | Top-level orchestrator (the analogue of `mass.rs`). Selects ordinarium template per hour, drives `specials()` rewriting, calls psalmody / hymn / capitulum / oratio / canticum sub-pipelines. Mirror of Perl `horas.pl` (733) + `specials.pl` (813). | **~2,800 LOC** |
| `src/horas/psalter.rs` | Loader and selector for the weekly psalter (Roman, Monastic, post-DA, post-Pius-X "Pius XII Bea"). Mirror of Perl `specials/psalmi.pl` (692). | ~900 LOC |
| `src/horas/matins.rs` | Lessons (Lectio 1–9), Invitatorium, hymn-shifting (`hymnshift`/`hymnshiftmerge`/`hymnmerge` from `Directorium.pl`). Mirror of `specmatins.pl` (1,857). | ~1,400 LOC |
| `src/horas/hymnus.rs` | Hymn selection per hour incl. doxology rules and tempora overrides. Mirror of `specials/hymni.pl` (163). | ~250 LOC |
| `src/horas/capitulum.rs` | Chapter / responsory short / versicle. Mirror of `specials/capitulis.pl` (255). | ~350 LOC |
| `src/horas/oratio.rs` | Collect + commemorations + suffrage + dirge. Mirror of `specials/orationes.pl` (1,215). | ~1,400 LOC |
| `src/horas/preces.rs` | Preces feriales / dominicales eligibility (a Boolean predicate) + body emission. Mirror of `specials/preces.pl` (105). | ~200 LOC |
| `src/horas/prima.rs` | Prime is special: martyrology block, lectio brevis, “De Officio Capituli”, regula. Mirror of `specials/specprima.pl` (367). | ~500 LOC |
| `src/horas/canticum.rs` | Benedictus / Magnificat / Nunc Dimittis with tone selection and Advent O-antiphons. Lifted from `horas.pl:505-575`. | ~250 LOC |
| `src/horas/concurrence.rs` *(or fold into precedence.rs)* | First-vespers / second-vespers split — see §4. | ~600 LOC |
| `src/horas/martyrologium.rs` | `Martyrologium*/` lookup keyed by movable feast date table. | ~250 LOC |
| `src/horas/ordinarium.rs` | Parser for the `Ordinarium/{Matutinum,Laudes,Prima,Minor,Vespera,Completorium}.txt` skeletons, including the `(sed rubrica X)` conditional-line stripper. Mirrors `specials.pl:specials()` skeleton walker (lines 1-200). | ~400 LOC |

### New data-pipeline code

| Path | Purpose | Est. LOC |
|---|---|---|
| `data/build_horas_json.py` | Parallel of `build_missa_json.py`: walks `horas/Latin/{Tempora,Sancti,Commune,Psalterium,Ordinarium}` and emits `data/horas_latin.json` (≈ 12 MB raw → ~6 MB postcard, vs Mass at 2.6 MB). | ~700 LOC |
| `data/build_psalms_json.py` | Pulls psalm bodies from `horas/Latin/Psalterium/Psalmorum/Psalm{1..150}.txt` plus the canticles (#229–234, #210–226 numbered slots) and `Invitatorium.txt`, into a flat keyed JSON. | ~250 LOC |
| `data/horas_latin.json`, `data/psalms_latin.json` | Build outputs. |

---

## 2. Hour-by-hour shape

The CGI dispatch is `horas($hora)` in `horas.pl:28`. Every hour
follows the same outer skeleton (the `Ordinarium/*.txt` file), then
the `specials()` walker in `specials.pl` rewrites each `#Section`
header into actual content from the day's office file +
`Psalterium/Special/*.txt` + `Commune/*.txt` chain.

Output shape for every hour: a sequence of typed blocks
(antiphon, psalm-body, hymn-strophe, capitulum, versicle/response,
prayer, conclusion). All hours share `&Deus_in_adjutorium` →
`#Psalmi` → `#Capitulum/Hymnus/Versus` → `#Oratio` → `#Conclusio`
modulo per-hour tweaks.

| Hour | Driver | Skeleton file | Distinctive sections | Sources required |
|---|---|---|---|---|
| **Matutinum** | `specmatins.pl` | `Ordinarium/Matutinum.txt` | Invitatorium (Ps 94 + special antiphon), 1/3/9 nocturns × 3 psalms × 3 lectiones, `Te Deum`, hymn-shift logic. | `Psalterium/Psalmi/Psalmi matutinum.txt` (psalter index by `[Day0]`–`[Day6]`), per-day antiphons in feast Tempora/Sancti file (`Ant Matutinum`), Lectios 1–9 in feast file or scripture-rotation table from `initiarule()`. |
| **Laudes** | `horas.pl` + `specials/psalmi.pl::psalmi_major` | `Ordinarium/Laudes.txt` | 5 psalms (one is OT canticle #210–216), `Benedictus` (canticum 230), Suffragium (omitted post-1955). | `Psalterium/Psalmi/Psalmi major.txt` `[Day0 Laudes1]`/`[Day0 Laudes2]`/`[Daya0 Laudes]`/`[DayaC Laudes]` (Roman vs Pius-X vs feast vs Paschal feria); per-day antiphons in Sancti/Tempora; `Hymnus Laudes`. |
| **Prima** | `specials/specprima.pl` | `Ordinarium/Prima.txt` | Hymn `Jam lucis`, 3 (or 4) psalms, capitulum versum, **Martyrologium**, **De Officio Capituli** (1960 only), Regula (Monastic), Lectio brevis, conclusion `Adjutorium nostrum`. | `Psalterium/Psalmi/Psalmi minor.txt` `[Prima]` block; `Psalterium/Special/Prima Special.txt`; Martyrologium per date. |
| **Tertia / Sexta / Nona** | `specials/psalmi.pl::psalmi_minor` | `Ordinarium/Minor.txt` (shared) | Hymn (`Nunc Sancte`/`Rector potens`/`Rerum Deus tenax`), 3 psalms, capitulum, short responsory, versicle, oratio. | `Psalmi minor.txt` `[Tertia]`/`[Sexta]`/`[Nona]` blocks; `Minor Special.txt` for capitulum + responsory; tempora-specific antiphon overrides (Adv, Quad, Pasc tables). |
| **Vespera** | `horas.pl` + `psalmi_major` | `Ordinarium/Vespera.txt` | 5 psalms, `Magnificat` (canticum 231), Suffragium pre-1955. Concurrence rewriting toggles `Capitulum Vespera 1` vs default. | `Psalmi major.txt` `[Day0 Vespera]`–`[Day6 Vespera]`; Sancti/Tempora `Hymnus Vespera`, `Ant Vespera`/`Ant Vespera 3`, `Capitulum Vespera`. |
| **Completorium** | `specials/psalmi.pl` minor branch + `horas.pl::canticum` | `Ordinarium/Completorium.txt` | Lectio brevis, Confiteor, hymn `Te lucis ante terminum`, 3 psalms, `Nunc Dimittis` (canticum 232), conclusion + Marian antiphon. | `Psalmi minor.txt` `[Completorium]` block; `Minor Special.txt` for `Lectio Completorium`, `Ant 4*`, `Versum 4`; `Mariaant.txt` for the seasonal closing antiphon. |

---

## 3. The psalter

Psalms 1-150 themselves live in `Psalterium/Psalmorum/Psalm{N}.txt`
(202 files including a- and b-split forms). Each file is an
N-language polyglot — Latin Vulgate, Latin-Bea (Pius XII), English,
French, German, etc. — concatenated. The Rust loader pulls only the
Latin block (and Latin-Bea when the `psalmvar` flag is set).

The **weekly distribution** (which psalm goes at which hour on
which weekday) lives in three index files:

- `Psalterium/Psalmi/Psalmi matutinum.txt` — Matins. Sections
  `[Day0]` (Sunday) through `[Day6]` (Saturday), each listing antiphon
  → psalm groups for the 3 nocturns. `[Day31]` is the
  reformed/abbreviated 1911 ordering for Sunday Matins under DA.
- `Psalterium/Psalmi/Psalmi major.txt` — Lauds + Vespers. Sections
  `[Day0 Laudes1]` (Sunday-Lauds-Schema-1, traditional with Ps 92),
  `[Day0 Laudes2]` (penitential schema with Ps 50), `[Day0 Vespera]`,
  through Day6. **Plus** Paschal-tide variants
  (`[Daya0 Laudes]`/`[DayaC Laudes]`/`[DayaP Laudes]`/`[Daya1
  Laudes]`-`[Daya6 Laudes]`), Monastic variants (`[Monastic Laudes]`,
  `[Monastic Vespera]`), and Cistercian variants
  (`[Cistercian Laudes]`).
- `Psalterium/Psalmi/Psalmi minor.txt` — Prime/Terce/Sext/None/
  Compline. Sections `[Prima]` (with `Dominica` / `Feria II` …
  `Sabbato` keyed inside), `[Tertia]`, `[Sexta]`, `[Nona]`,
  `[Completorium]`, plus Tempora antiphon overrides (`[Adv1]`,
  `[Quad1]`, `[Pasc]`, …) and the Monastic/Tridentine separate
  schemata at top (`Monastic = …`, `Tridentinum = …` keyed values).

So **one psalter file per "hour-class" (Matins / Major / Minor)**
holds a *layered* index: keyed first by liturgical version, then by
weekday, then by Lauds-schema-1-vs-2 / Paschal flag / feast-class.
The runtime selector (`psalmi_minor` / `psalmi_major` / `psalmi_
matutinum`) decides which key to read by inspecting `$version`,
`$dayofweek`, `$winner{Rank}`, `$rule`, `$dayname[0]` (the season).

**Cursus differentiation:**

- **Roman (Tridentine 1570 → Divino Afflatu 1911)** — default.
  `$version =~ /Trident/` selects the `Tridentinum` keyed section
  inside the file (`[Tertia]: Tridentinum=...`); Roman post-DA
  selects the unprefixed `[Day*]` blocks.
- **Pius-X 1911 schema** — already encoded in the same `Psalmi
  major.txt` blocks. The Tridentine variant uses the older Sunday
  Lauds-schema-1 (`117` at Lauds, `92` at Compline-Sunday); DA uses
  the schema-2 split.
- **Monastic (1617 / Divino / 1963)** — `$version =~ /Monastic/i`
  branches into `psalmi{Monastic}` (Monastic top-level key in
  `Psalmi minor.txt`) and `[Monastic Laudes]`/`[Monastic Vespera]`/
  `[DaymF Laudes]`/`[DaymP Laudes]`/`[Daym6F Laudes]`/`[DaymF
  Canticles]` in `Psalmi major.txt`. Out-of-scope for the first
  parity pass since the Mass-side port doesn't ship Monastic
  either.

The 21 antiphon-set files (`[Adv1]`-`[Pent01]`-`[Pasch]` etc. inside
`Psalmi minor.txt`) are tempora-keyed antiphon overrides applied on
top of the per-day-of-week defaults; the selector is
`gettempora('Psalmi minor')` (Perl `horascommon.pl`).

Net data-pipeline work for the psalter: a single
`build_psalms_json.py` that ingests the 202 `Psalm{N}.txt` files
and the three index files, plus
`Psalterium/{Invitatorium,Doxologies,Mariaant,Benedictions,Chant}.txt`,
into one keyed JSON. The four `Special/*.txt` files
(189+147+27+71 = 434 sections) feed
hymns/capitula/responsories per season.

---

## 4. Concurrence + first vespers

`occurrence(date)` answers "what is the office of *this* day?".
The Breviary additionally needs **`concurrence(today, tomorrow)`**
for Vespers and Compline: these two hours straddle the calendrical
day boundary and may belong wholly to *tomorrow*'s office, wholly
to *today*'s, or a blended "a capitulo de sequenti" (Vespers
ordinary up to the Capitulum from yesterday, Capitulum + Hymn +
Magnificat from tomorrow's first-Vespers, then both yesterday's
and tomorrow's collects as commemorations).

Perl mechanics (`horascommon.pl:810-1426`):

1. Call `occurrence(...,$tomorrow=1)` to compute `cwinner` /
   `crank` / `ccommemoentries`. Cache today's via a second
   `occurrence(...,0)`.
2. Compare `wrank[2]` vs `cwrank[2]` with **rubric-conditional
   "flattened ranks"**: under Tridentine, all minor doubles below
   2.99 are flattened to rank 2 for comparison; under Divino,
   Sunday-vs-double has a different ladder; under 1960, equal
   ranks are resolved in favour of **today**.
3. Set the global `$vespera ∈ {1,3}` (1 = first Vespers of
   tomorrow, 3 = second Vespers of today) and either drop
   `cwinner` (today wins, no commemoration of tomorrow) or rotate
   `winner ↔ cwinner` (tomorrow wins, today survives as
   commemoration).
4. The "a capitulo" branch (`flcrank == flrank`) sets a special
   `$antecapitulum` global so the `Capitulum Hymnus Versus`
   section uses tomorrow's body even though the psalmody is
   today's.

In the existing Rust `OfficeOutput.vespers_split` field is already
reserved (see `precedence.rs:60-66`). The proposed shape:

```rust
pub struct VespersSplit {
    pub kind: VespersKind,        // FirstVespers / SecondVespers / ACapitulo
    pub psalmody_from: FileKey,   // today or tomorrow's office
    pub capitulo_from: FileKey,   // for ACapitulo: tomorrow; else == psalmody_from
    pub commemoratio: Vec<FileKey>,
    pub headline: String,         // "Vespera de sequenti; commemoratio de praecedenti"
}
```

Compute it from a single `concurrence()` function that calls
`compute_occurrence` twice (today, tomorrow) and runs the
flattened-rank ladder. **The `occurrence` engine itself doesn't
change — concurrence sits one layer above.** This is the cleanest
factorization, and the Mass-side port already keeps `occurrence`
pure of concurrence concerns.

---

## 5. Estimated LOC + weeks

### LOC

| Bucket | Mass port (today) | Breviary port (estimate) |
|---|---|---|
| Lib code (Rust) | 12,300 | **+9,500–11,000** (`horas.rs` ≈ 2,800; sub-modules ≈ 6,200; precedence/concurrence extension ≈ 600; data-types/corpus ≈ 200; everything else folded into existing) |
| Data-pipeline (Python) | 978 | **+1,000** (`build_horas_json.py` ≈ 700, `build_psalms_json.py` ≈ 250) |
| Test/regression harness | 1,946 (mass-only) | **+1,500** (8-hour cell extractor + per-hour comparators + new year-sweep binary `breviary_year_sweep.rs`) |
| Total | ~15,000 | **~27,000–28,500** (≈ +12k LOC) |

Sanity-check vs. Perl: the Breviary Perl is 3.7× the size of the Mass
Perl (10,572 vs 2,843 LOC). Our Mass port came in at 4,109 LOC of
`mass.rs` + ~1,500 LOC of supporting code = ~5,600 LOC for ~2,843 LOC
of Mass Perl, so a 2:1 expansion ratio. Applying the same ratio to
the Breviary's ~7,700 net new Perl LOC (10,572 − 2,843 reused) gives
~15,400 LOC of new Rust — close to the 11k estimate above, modulo
how much of `horascommon.pl` is already ported. Call it
**11k–15k Rust LOC** with a wider band.

### Calendar time

Single full-time engineer pair-programming with this codebase as the
seed:

- Phase 1 (psalter + one hour): **3 weeks**
- Phase 2 (5 remaining hours): **5 weeks**
- Phase 3 (Matins): **3 weeks** (lessons rotation is the messiest)
- Phase 4 (concurrence + 1st Vespers): **2 weeks**
- Phase 5 (regression sweep to 100 % parity, all 5 rubric layers,
  parity for the 4 we already ship): **3 weeks**

**Total: ~16 weeks (4 months)** to reach Mass-equivalent parity. Doubles to ~8 months
under realistic part-time contention with rubric edge cases.

---

## 6. Stage plan

### Phase 1 — psalter loader + Vespers (3 wks)

- Build `data/psalms_latin.json` (the 150 psalms + canticles +
  invitatorium).
- Build `data/horas_latin.json` (Tempora + Sancti + Commune for
  Breviary).
- Implement `src/horas/psalter.rs` (`psalmi_major` for Vespers
  only; defer Monastic).
- Implement `src/horas/ordinarium.rs` (skeleton walker).
- Implement `src/horas/hymnus.rs`, `src/horas/capitulum.rs`,
  `src/horas/oratio.rs`, `src/horas/canticum.rs`.
- Wire `src/horas.rs::vespers(input, corpus) -> Office` and
  expose `Office { sections: Vec<OfficeBlock> }` over WASM
  alongside `MassPropers`.
- Smoke-test: render Vespers for 12 hand-picked dates (Christmas,
  Epiphany, Quad1 Sunday, Pasc0, Pent0, common feria, Duplex I cl.)
  vs. the Perl oracle.

### Phase 2 — Lauds + Prime + Terce/Sext/None + Compline (5 wks)

- Lauds: extend `psalmi_major` to handle Lauds-schema-1/2 split,
  Paschal `[Daya*]` overrides, and the Sunday `Daya0` vs `DayaC`
  branch.
- Prime: implement `src/horas/prima.rs` (`specprima.pl` 367 LOC,
  the densest hour). Includes martyrology lookup
  (`src/horas/martyrologium.rs`).
- Terce/Sext/None: thin wrappers over `psalmi_minor` + shared
  `Minor.txt` skeleton.
- Compline: separate skeleton, includes the Marian closing antiphon
  selector (Alma Redemptoris / Ave Regina / Regina Caeli /
  Salve Regina).
- Year-sweep regression for 365 days × 6 hours × 5 rubrics =
  10,950 cells, target ≥99 %.

### Phase 3 — Matins (3 wks)

The single biggest hour. Implement `src/horas/matins.rs` against
`specmatins.pl` (1,857 LOC):

- Invitatorium with seasonal antiphon selector.
- Nocturn count: 1 (festal) / 3 (Sundays + I cl.) / 9 (Tridentine
  pre-1911 ferials kept all psalms).
- Lectio rotation table (`initiarule()`) — Scripture-of-the-day
  for ferias, sometimes shifted forward when a feast displaces
  the proper Lectio of a Sunday.
- Hymn-shift / hymn-merge (`hymnshift`/`hymnshiftmerge`) for
  Christmastide / Epiphanytide.
- `Te Deum` suppression on penitential days.
- Add Matins to the regression sweep → 14,600 cells × 5 rubrics.

### Phase 4 — Concurrence + first Vespers (2 wks)

- Implement `src/horas/concurrence.rs` (or fold into `precedence`).
- Wire `OfficeOutput.vespers_split: Some(VespersSplit { … })`.
- Cover the four Vespers/Compline cases: SecondVespers (today
  outranks tomorrow), FirstVespers (tomorrow outranks today),
  ACapitulo (equal flattened ranks), and the rubric-conditional
  "Simplex / vigil / infra octavam suppression" branches.
- Specifically test the high-traffic concurrences: Sat→Sun
  Vespers, Dec 31 → Jan 1, Saturday→Saturnine BVM,
  Pasc6-6→Pasc0, Pent6-6→Pent0.

### Phase 5 — Year-sweep regression to 100 % parity (3 wks)

- Extend `regression.rs` cell extraction to all 8 hours.
- New binary `src/bin/breviary_year_sweep.rs` parallels
  `year_sweep.rs`, comparing Rust output blob-for-blob against the
  Perl oracle (`scripts/do_render.sh` will need a `--breviary`
  flag and an hour selector).
- Drive 4 rubric layers (1570 / Divino / 1955 / 1960) to 100 %,
  R60 to ≥99.7 % matching the current Mass parity bar.
- Catalogue divergences in a new
  `docs/UPSTREAM_WEIRDNESSES_BREVIARY.md`.

---

## 7. Risks + open questions

1. **The `getproprium` chain has 4 fallback levels** — winner →
   commune → tempora → psalterium-special. Mass uses 2. The
   Breviary fallback is per-section-key (`Ant Vespera` falls
   through to `Ant Laudes`; `Hymnus Vespera 3` to `Hymnus
   Vespera`; `Capitulum Laudes` is shared with `Capitulum
   Vespera`). The Mass-side `proper_block` resolver doesn't model
   this depth; need a Breviary-specific resolver in
   `src/horas/proper.rs`. **Low risk, well-bounded.**

2. **`@`-references with section substitution** — Breviary uses
   `@:Section in 4 loco s/PAT/REPL/` substitutions far more
   heavily than Mass. The Mass port currently defers these and
   only handles the "rare in 1570" case (per `mass.rs:14`). Phase
   1 will need the substitution form working; estimate +200 LOC
   in `horas/proper.rs`. **Medium risk** — there's an open issue
   about which Sancti files use it and how the Perl regex flags
   interact with Unicode antiphon text.

3. **Hymn doxology selection** is a 4-state lookup
   (`doxology()` in `specials/hymni.pl`): per-season per-rubric,
   with a fallback chain. 1962 stripped doxologies entirely. The
   selector reads `dayname[0]`, `version`, `winner{Rule}`,
   `commune` — Mass doesn't need any of this. **Low risk,
   well-documented in Perl.**

4. **GABC chant tones** — every antiphon and capitulum carries an
   optional `;;<tone>` suffix used only when `lang =~ /gabc/`.
   The Latin-only port can drop this on the floor, but the build
   pipeline must preserve it for any future GABC layer.
   **No risk for Latin parity**, but locks in a data-model
   decision now.

5. **Pius-X (`Latin-Bea`) variant text for psalms** — when
   `psalmvar` is set, the renderer swaps Latin → Latin-Bea for
   the psalter. Bea's text is shipped in the same polyglot
   `Psalm{N}.txt` files with a `(Messias rex…)` marker (visible
   in `Psalm109.txt`). Either ship both or feature-gate.
   **Low risk** (purely a data toggle), but worth deciding in
   Phase 1.

6. **Martyrologium** is its own beast: a separate corpus
   (`Martyrologium*/`) keyed by month-day plus paschal-day shift
   plus saint-year cycles, computed via DiPippo's golden-number
   tables. The current Mass port doesn't touch it. Estimate
   ~250 LOC for `martyrologium.rs` + a separate
   `data/build_martyrologium_json.py`. Used only at Prime, but
   completely independent of psalter logic. **Medium risk** —
   the tabula litterarum martyrologii is a piece of pre-Vatican-II
   computational astronomy in itself; DiPippo > Perl when the
   two diverge.

7. **Suffragium / Octave commemorations** were progressively
   abolished (1955 trimmed Octaves, 1960 abolished Suffragium).
   The pre-1955 Suffragium logic in `oratio.pl` is non-trivial
   (Ad Crucis / De BMV / De Apostolis cycle). For 1960-only
   parity this drops out; for 1570/Divino parity it's required.
   **Medium risk** for pre-DA layers.

8. **Office of the Dead, Office of Mary, Vigil-day variants** —
   votive offices (`votive=C9` for Defunctorum, `C12` for BMV
   parva) are toggled by the user, not by date. They short-circuit
   `precedence()` entirely and pull text from
   `CommuneM/C9*.txt` / `C12*.txt`. **Low risk** if treated as a
   separate output mode rather than woven into the main pipeline,
   but easy to forget at the API-surface level.

9. **Exact white-space / line-break parity** with the Perl
   renderer — already a known pain point on the Mass side
   (`UPSTREAM_WEIRDNESSES.md`). The Breviary's hymn strophes have
   their own `_\n` separator convention and the responsory body
   has triple-line `R.br./V./R./Gloria/R.` splits that the
   regression comparator must normalize identically across both
   sides. **Will require comparator extensions; budget already
   absorbed in Phase 5.**

10. **Same-pure-core architecture preference** — per the user's
    `feedback_divinum_officium_port.md`: 1570 baseline + composable
    reform layers, no globals, gitignored Perl vendor, DiPippo as
    tiebreaker. The Breviary port must follow the same pattern
    rather than mirroring Perl's `our $hora`/`@dayname`/`%winner`
    global thrash. The sub-module structure proposed in §1 keeps
    each function pure over `(office: &OfficeOutput, hour: Hour,
    corpus: &dyn Corpus) -> Vec<OfficeBlock>`. Risk if any helper
    accidentally pulls in `thread_local!` state — flag in code
    review.
