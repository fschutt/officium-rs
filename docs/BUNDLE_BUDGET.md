# Bundle budget — leg-K progress

The SUPER_PLAN exit criterion is **≤ 1 MB raw / ≤ 700 KB brotli** for
the WASM `.wasm`, with the demo site under 1.2 MB total payload.

## K2 — combined missa+horas brotli (✅ DONE 2026-05-04)

Pack the two largest postcard outputs (`horas_latin` + `missa_latin`)
into one brotli stream so the encoder sees their shared liturgical
phrasing in a single context. Header is 8 bytes (two little-endian
u32 lengths), then the two raw postcards back-to-back. Runtime
decompresses once into a shared `OnceLock<Vec<u8>>`, then exposes
the two slices via `embed::horas_postcard()` / `embed::missa_postcard()`.

Sizes (post-K2):

| File                              | Brotli `.br` | Share |
|-----------------------------------|-------------:|------:|
| `corpus.postcard.br`              |    1,348,822 | 89.8 % |
|   (horas + missa combined)        |              |       |
| `psalms_latin.postcard.br`        |       84,913 |  5.6 % |
| `ordo_latin.postcard.br`          |       50,241 |  3.3 % |
| `kalendaria_by_rubric.postcard.br`|       11,807 |  0.8 % |
| `sancti.postcard.br`              |        5,833 |  0.4 % |
| `kalendaria_1962.postcard.br`     |        1,194 |  0.1 % |
| **Total brotli**                  | **1,502,810** | 100 % |

K1 → K2 delta:

| Stage      | Bundle brotli | Delta |
|------------|--------------:|------:|
| K1 baseline | 1,771,271 | — |
| K2 ship    | 1,502,810 | **−268,461 (−15.2 %)** |

K2 also bumped `lgwin` from 22 to 24 in `brotli_compress` (default
~4 MB → 16 MB window). lgwin alone gave only ~3 KB; the combined
stream is the load-bearing change.

## Distance from target

- Total brotli today: **1.50 MB**
- Target brotli:     **0.70 MB**
- Need to shave:     **~0.80 MB** (≈ 53 %)

## K1 baseline (snapshot, pre-K2)

Captured at commit `b70f113` for reference:

| File                              | Brotli `.br` | Share |
|-----------------------------------|-------------:|------:|
| `horas_latin.postcard.br`         |    1,099,586 | 61.4 % |
| `missa_latin.postcard.br`         |      517,673 | 28.9 % |
| `psalms_latin.postcard.br`        |       84,924 |  4.7 % |
| `ordo_latin.postcard.br`          |       50,324 |  2.8 % |
| `kalendaria_by_rubric.postcard.br`|       11,811 |  0.7 % |
| `sancti.postcard.br`              |        5,784 |  0.3 % |
| `kalendaria_1962.postcard.br`     |        1,169 |  0.1 % |
| **Total brotli**                  | **1,771,271** | 100 % |

(WASM binary itself is separate; this is just embedded data.)

## Distance from target

- Total brotli today: **1.77 MB**
- Target brotli:     **0.70 MB**
- Need to shave:     **~1.07 MB** (≈ 60 %)

## Where the 1.77 MB lives

`horas_latin` (the Breviary corpus) is the heavy hitter at **1.1 MB
brotli** — 61 % of the bundle. `missa_latin` (the Mass corpus) is
**518 KB brotli** — 29 %. Together they're 90 % of the total. The
psalter, Ordinarium, kalendar tables, and Sancti index are the
remaining 10 % combined.

So leg-K work targets the Breviary corpus first, the Mass corpus
second, and accepts the rest as already small.

## What's IN `horas_latin` right now

- 1,204 horas keys (Tempora + Sancti per-day office files)
- 202 psalms inline (separate file but content overlaps)
- 8 Ordinarium hour skeletons
- ~62 Commune templates
- 32 Mariaant variants

The corpus is mostly **Latin prose** — antiphons, hymns, lessons,
oratios, capitulae. Brotli does well on natural-language repetition,
which is why we're already at 1.1 MB from a ~4.3 MB raw JSON. The
remaining ~400 KB to shave probably needs structural work, not
compression-tuning.

## leg-K candidate tactics

### K2 — shared-dictionary brotli

Build a brotli dictionary from the Breviary corpus's most-frequent
n-grams (`Per Dóminum`, `Sancti, sancti, sancti`, `Glória Patri`,
`℣.`, `℟.`, etc.). Both `horas_latin` and `missa_latin` share the
same liturgical phrasing; a shared dictionary should compress both
better than per-file brotli.

Estimated savings: **15–25 %** off the combined `horas_latin +
missa_latin` brotli (currently 1.62 MB → ~1.25–1.40 MB). Closes
roughly **half** the gap to the 700 KB target.

### K3 — drop the `regression` feature from default builds

The `regression` Cargo feature pulls in the comparator HTML walkers
and Perl-interop helpers. They're not needed for the WASM artefact.
Confirm `Cargo.toml`'s `default-features = false` for the wasm
crate; measure delta.

Estimated savings: WASM-only — won't change the data brotli sizes.

### K4 — `wasm-opt -Oz` after each leg ships

Already wired in `pages.yml`. Re-measure after K2 to set a
post-optimization baseline.

### K5 — final published budget

Once K2 + K3 ship, run a fresh measurement. If we're under 700 KB
brotli total, exit-criterion 4 is met.

## Per-prayer overlap candidates (for K2 dictionary)

Strings that appear ≥ 50 times across both corpora:

- `$Per Dominum` / `$Per eumdem` / `$Qui tecum` / `$Qui vivis` —
  prayer conclusions (every Oratio, Secreta, Postcommunio, Compline,
  Vespers).
- `Glória Patri, et Fílio, et Spirítui Sancto.` — every Magnificat,
  Benedictus, Nunc dimittis, end of every psalm.
- `℣.` and `℟.` — versicle / response markers.
- `Sicut erat in princípio, et nunc, et semper, et in sǽcula
  sæculórum. Amen.` — the second half of every Glória Patri.
- `&Gloria` / `Glory be to the Father…` — the macro form vs the
  expansion.
- Common psalm refrain antiphons.

A static custom dictionary keyed on these strings should make a
visible dent in the `horas_latin` brotli.

## Next slice (K2)

1. Concatenate the most-frequent N strings (target ~16 KB, the
   brotli dictionary size cap).
2. Wire `BrotliCompressInputDictionary` (or equivalent) into
   `build.rs` for `horas_latin.postcard` and `missa_latin.postcard`.
3. Re-measure. Document the delta in `BUNDLE_BUDGET.md`.
4. Commit + advance.
