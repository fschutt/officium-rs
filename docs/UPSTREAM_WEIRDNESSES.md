# Divinum Officium upstream — observations from the Rust port

Notes for the upstream Perl maintainers about anomalies encountered
while porting the propers resolver to Rust. Most are not bugs per se
(the Perl runtime "just works" because it threads context through
globals), but they are surprises that an external reader / ports
author will hit.

## 1. `Pasc2-3.txt` is "Patrocinii St Joseph" — file-stem doesn't match week-day

`/tmp/do-upstream/web/www/missa/Latin/Tempora/Pasc2-3.txt`:

```
[Officium]
Patrocinii St. Joseph Confessoris Sponsi B.M.V. confessoris

[Rank]
;;Duplex I classis;;6;;
```

The file-stem `Pasc2-3` reads as "Wednesday in 2nd week of Easter",
but the file actually carries the **Patrocinii St Joseph** Mass
(itself a movable feast on the 3rd Wednesday after Easter
historically, instituted 1847, abolished 1956). Surrounding files
`Pasc2-4.txt` … `Pasc3-3.txt` form the Octave (`Die II..VIII infra
Octavam Patrocinii`) but the bare-stem ferias for those days are
`Pasc2-3Feria.txt` … `Pasc3-3Feria.txt`. Confusing for ports — the
expectation from the stem alone is that `Pasc2-3` carries the feria
and `Pasc2-3Feria` would be a transferred/variant form.

## 2. `Tempora/093-3.txt`, `093-5.txt`, `093-6.txt`, `104-0.txt` —
   day-of-year numeric stems

```
$ ls Tempora/ | grep -v "^[A-Z]"
093-3.txt   ← Sabbato Quattuor Temporum Septembris (?)
093-5.txt
093-6.txt
104-0.txt
```

These four files do not follow the dominant `<season><week>-<dow>`
naming. `093-6` is "Sabbato Quattuor Temporum Septembris" (Ember
Saturday in September); the leading number suggests day-of-year
(093 = April 3) but the contents suggest September Embertide. A
comment near the top of these files explaining the intended slot
would help.

## 3. `[Missa]\nName` directs to a Mass file by *title*, not file-stem

Multiple Common files declare e.g.:

```
[Missa]
Statuit
```

(`Commune/C2.txt` — Common of one Martyr Pontiff, base / non-paschal).
The Mass propers titled "Statuit" actually live in
`Sancti/01-22.txt` (S. Vincentii Mart.) — not in any
`Commune/Statuit.txt`. There's no in-tree explanation that the Mass
title resolves to a Sancti file with that title's incipit. A port
discovers this only by full-corpus grep on the Introit text.

(Similarly: `Commune/C2p.txt` says `[Missa]\nProtexisti`, which
resolves to `Sancti/04-25.txt` (St Mark Evangelist).)

## 4. Substring matching for "post-1570 octave" must be permissive

The Patrocinii octave officium spelling is inconsistent across files:
- `Pasc2-3.txt`: `Patrocinii St. Joseph Confessoris …` (with dot)
- `Pasc2-4.txt`: `Die II infra Octavam Patrocinii St Joseph` (no dot)
- `Pasc3-3.txt`: `Die Octava Patrocinii St Joseph`           (no dot)

A port that detects post-1570 octaves by substring `"Patrocinii St
Joseph"` will miss the Sunday file (`Pasc2-3`). Just match
`"Patrocinii"` — but it's a small footgun.

## 5. NFD ligature handling around `ǽ`

`Genetríce` / `Genetrícis` (with `í`) is one orthography and
`Genitrice` / `Genitricis` (with `i`) is another — they appear in
adjacent commune files (`Commune/C10.txt` vs `Commune/C11.txt`).
Even with rigorous Unicode normalization (NFD + combining-mark
strip), the bare `e` vs `i` letters survive — these are genuinely
different spellings of the same word. Worth deciding which is
canonical and applying corpus-wide; right now a regression
comparator sees these as divergent bodies.

## 6. Section-header annotations carry implicit corpus shape

Files like `Commune/C2b-1.txt` have **every** section annotated
`(communi Summorum Pontificum)`. This is the *only* hint that the
file has no Tridentine-1570 body of its own — it relies entirely on
its `@Commune/C2-1` parent inherit. A `[Description]` or `[Era]`
metadata block at the head of these files would make corpus
generations explicit, instead of having a port reverse-engineer the
"all-annotated → no-baseline-content" rule.

## 7. Saturday-BVM "free Saturday" detection in upstream `horascommon.pl:401-420`

The Perl rule fires on the saint's rank floor (~`< 1.4`). After
porting and adding the post-1570-octave rank-downgrade
(Sacred-Heart, Christ-the-King, Patrocinii — see #4), we still see
the Saturday-BVM rule firing on weekday-not-Saturday dates because
the kalendar override (`Tabulae/Kalendaria/1570.txt`) puts a
post-1570 saint on the date. The fix on our side is to treat the
1570 kalendar as authoritative; but the upstream rule itself works
off rubric-runtime state, which makes it hard to validate against a
single eternal source-of-truth.

## 8. Sancti/<MM-DD>.txt files reference non-existent commune stems

`Sancti/08-26.txt` (Zephyrinus, Pope-Martyr) declares
`[Rank];;Simplex;;1.1;;vide C2-1b`. There's no `Commune/C2-1b.txt` in
either `missa/Latin/Commune` or `horas/Latin/Commune`. The Perl
runtime silently falls back through `setupstring`'s "fall through to
horas" path; our reading is that it should be `vide C2b-1` (a
typo) or `vide C2-1` (a less-detailed reference). A typo audit
across the Sancti corpus would help — there are several similar
malformed `vide` columns.

## 9. `(rubrica tridentina)@Path` — undocumented conditional parent inherit

`Sancti/07-03oct.txt` opens with:

```
(rubrica tridentina)@Sancti/07-04oct

[Officium]
Quinta die infra Octavam Ss. Apostolorum Petri et Pauli
…
```

The first line is a conditional parent-inherit: under `(rubrica
tridentina)` the file's parent is `Sancti/07-04oct`. Other rubrics
get the file's own [Officium] / [Rank]. This is a real feature
(handled by `setupstring_parse_file`'s `vero()` predicate evaluator)
but not documented anywhere a port author can easily find. The
predicate-syntax library deserves a dedicated `docs/PARSER.md`.

## 10. Per-section conditional bodies via `(rubrica X)` annotations

Adjacent to #9: section bodies like

```
[Oratio] (rubrica divino aut rubrica 196)
…body for Divino+1960…
[Oratio]
…body for everything else…
```

are also evaluated against the `vero()` predicate. We currently model
this as a coarse "annotated_sections" list and skip post-1570
annotated sections in commune-fallback only — but the actual
semantics is: the section header's annotation is a runtime
expression. A list of recognised predicate names in one place would
help port authors validate they handle each.

## 11. `(sed rubrica 1570)` vs `(sed rubrica tridentina)` — same intent

Some Sancti files use `(sed rubrica 1570 aut rubrica monastica)` to
mark the 1570 variant of [Rank]. Others use `(sed rubrica
tridentina)`. They mean the same thing for our purposes (Tridentine
includes 1570 baseline) — but a port has to recognise BOTH spellings
or it loses Sancti/05-09's 1570 commune column.

## 12. `vide C2b-1` vs `ex C2b-1` semantics not clearly documented

`[Rank]` carries a `commune` column whose two values mean different
things:
- `vide C2b-1` = "see C2b-1 — some sections proper, others fall back"
- `ex C2b-1`   = "all sections drawn directly from C2b-1"

The actual behavior in `getproprium` (propers.pl) treats these
slightly differently (the `flag` parameter). A port that reads only
the file format would think they're synonyms.

## 14. Pent01-0 (Trinity Sunday) self-references itself → Perl infinite recursion

`Tempora/Pent01-0.txt` (Dominica Sanctissimæ Trinitatis under
Tridentine 1570) has:

```
[Introitus]
@Tempora/Pent01-0:Introitus
```

This is a literal self-reference. Perl's `setupstring` follows the
`@`-reference, lands in the same file's `[Introitus]` body, and
recurses indefinitely until it crashes with the error message:

> Cannot resolve too deeply nested Hashes

We see this verbatim in the rendered HTML for May 31, 2026 (Trinity
Sunday) under Tridentine - 1570.

The proper Trinity Sunday Introit ("Benedícta sit sancta Trínitas
atque indivísa Unitas", Tob 12:6) lives in
`Tempora/Pent01-0r.txt`, which has identical [Officium] /
[Rank] / [Oratio] but a real [Introitus] body. The
`Tabulae/Tempora/Generale.txt` redirect table redirects
`Pent01-0 → Pent01-0r` for **1960 Newcal** but not for Tridentine.

This means: **under Tridentine 1570 + Trinity Sunday, the upstream
Perl renderer crashes on the Introit**, while our Rust port avoids
the loop and falls through to the `-r` sibling variant — emitting
the correct Trinity Introit. The regression harness flags this as
"Differ" (perl text = error message vs rust text = real Introit),
but Rust's behavior is the desired output.

Recommended upstream fix: change `Pent01-0.txt:[Introitus]` to
`@Tempora/Pent01-0r:Introitus`, OR add a Tridentine-side redirect to
`Tabulae/Tempora/Generale.txt`:

```
Tempora/Pent01-0=Tempora/Pent01-0r;;Trident
```

## 13. Suffragium first-block layout is not deterministic from `Suffr=…`

The `Suffr=Maria3;Ecclesiæ,Papa;;` directive in `[Rule]` controls
which Suffragium prayers get appended to Oratio / Secreta /
Postcommunio. Each `;`-group is rotated by `dayofweek % len`, the
chosen entry looks up `<sect> <name>` in `Ordo/Suffragium.txt`, and
the body is `delconclusio`'d before being concatenated with `_\n`
separator. Final `$addconclusio` is appended at the end.

In the rendered HTML the comparator's "first-Oratio-block-wins" rule
sees different layouts on different days even with the same `Suffr=`
form:

* **Sat-BVM 2026-06-13** (winner Commune/C10c, [Rule] from C10
  `Suffr=Spiritu;Ecclesiæ,Papa;;`): the first Oratio block is JUST
  the main "Concede nos famulos tuos" + Per Dominum. The Suffragium
  prayers (Spiritu, Ecclesiæ, Papa) are in a SEPARATE second
  Oremus block.

* **Pasc6-1 2026-05-18** (winner Tempora/Pasc6-1, [Rule]
  `Suffr=Maria3;Ecclesiæ,Papa;;`): the first Oratio block contains
  main "Concede quaesumus" + a `Pro Papa` rubric (with capital P,
  not the lowercase `Pro papa` from Suffragium.txt) + a Pope-Oratio
  body + Per Dominum. The Suffragium-rotated `Maria3 + Papa` are
  in a SEPARATE second Oremus block.

The TWO `Pro Papa` blocks rendered for Pasc6-1 (one capitalised in
the first block, one lowercase in the second) appear to come from
two different sources:
1. A `commemoratio1` set somewhere in horascommon.pl that fires for
   Octave-of-Ascension days specifically (the source of the
   capitalised "Pro Papa" inside the first block).
2. The Suffragium loop firing as documented (the lowercase
   "Pro papa" in the second block).

A port that just implements the documented `Suffr=…` algorithm
matches Sat-BVM (where there's no extra commemoratio1) but misses
the Pasc6-1 case. We disable the append in `mass.rs` for now and
leave it as a known divergence (3 cells out of 4380).

The deeper weirdness: the Perl rendering doubles up "Pro Papa"
because the same Pope is commemorated by *two* different mechanisms
that aren't deduplicated.
