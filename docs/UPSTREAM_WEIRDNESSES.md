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

## 17. `Sancti/10-07.txt` references `@Sancti/9-12:Evangelium` (single digit)

`vendor/divinum-officium/web/www/missa/Latin/Sancti/10-07.txt`:

```
[Evangelium]
@Sancti/9-12:Evangelium
```

The corpus filename is `Sancti/09-12.txt` (zero-padded month),
but the reference is `9-12` (no padding). Perl's
`SetupString::checkfile` does a literal filesystem lookup, finds
no `Sancti/9-12.txt`, and renders the placeholder text:

```
Sancti/9-12:Evangelium is missing!
```

— literally inlined into the Latin Mass output. The English
sibling file `English/Sancti/10-07.txt` ships the body inline
(Luke 1:26-38, the Annunciation Gospel) so the English column
renders correctly while Latin breaks.

Affects October-DP (Solemnitas Rosarii on the 1st Sunday of
October, which inherits via `@Sancti/10-07`) under every
Tridentine version where the feast is celebrated. Last
T1910/DA blocker for 100%.

Fix would require either: (a) upstream patching the reference
to `09-12`, (b) Rust normalising single-digit stems on the way
into chase_at_reference (Rust would then render the *correct*
gospel while Perl still prints the placeholder — diff persists),
or (c) Rust mimicking Perl's "is missing" placeholder (silly).

## 18. `[Rank] (rubrica Trident)` second-Rank header on horas Sancti/10-DP

`vendor/divinum-officium/web/www/horas/Latin/Sancti/10-DP.txt`:

```
[Rank] (rubrica Trident)
In Sollemnitate Rosarii Beatæ Mariæ Virginis;;Duplex 2 classis;;5.1;;ex C11
(sed rubrica cisterciensis)
Solemnitas SS. Rosarii Beatæ Mariæ Virginis;;Duplex 2 classis;;4.1;;ex C11
```

The annotation token `Trident` (no trailing -ina) is the regex
predicate `$version =~ /Trident/i` — matches every "Tridentine -
*" version (1570/1888/1906/1910) plus Monastic Tridentinum.
Different from the more common `tridentina` token (named
predicate). Both work but they aren't interchangeable across
the corpus.

## 19. `Sancti/10-DP.txt` is in `horas/` only, not `missa/`

`vendor/divinum-officium/web/www/missa/Latin/Sancti/10-DP.txt`
does NOT exist. Only `horas/Latin/Sancti/10-DP.txt` ships it.

The Solemnitas Rosarii feast (Pius X 1888 reform: 1st Sunday of
October) is invoked via the Transfer table:

```
Tabulae/Transfer/<letter>.txt:
10-04=10-DP;;1888 1906 C1951
```

So the kalendar wants Sancti/10-DP. Perl's `SetupString::checkfile`
cascade falls back from `missa/Latin` to `horas/Latin` when a
non-Commune file is missing — that's how it finds 10-DP. A port
that only walks `missa/Latin/Sancti/` will never see it, and the
Mass on the 1st Sunday of October falls through to the Tempora
Sunday Mass instead of the Rosary.

Other horas-only Sancti files: `04-01C.txt`, `06-29oct.txt`,
`07-16sab.txt`, `07-26n.txt`, `10-31v.txt`, `11-13n.txt`,
`12-09-da.txt`, `Quad5-6-Septem.txt`. The horas-only Tempora
tree is much larger — 158 files harvested, including Cisterciensis
and Monastic variants.

## 20. `(rubrica 1962)` doesn't apply to Rubrics 1960

The naive substring check "if `196` is in the predicate, it
applies to R60" is wrong. R60's `$version` is exactly "Rubrics
1960 - 1960" — the regex `/1962/` against this string is FALSE.
But the substring "196" *is* contained in "1962", so a port that
checks `"196" in label` over-matches.

Concretely, `(sed rubrica 1570 aut rubrica monastica aut rubrica
1962)` (Sancti/01-24 Timothy) populates only the T1570 and
Monastic Tridentinum 1617 buckets. Under R60 the default rank
applies — Timothy's Duplex 3.0 stays Duplex (= III. classis
under R60), beating the Saturday-of-BVM Marian Mass.

The 1962 token in Generale.txt and these mass files is the
Dominican rubric (`Ordo Praedicatorum - 1962`), which our port
doesn't yet target. Same for `1963` (Monastic 1963), `1965` etc.

Fix: per-predicate regex check against the active version
string, not substring-of-token. `(sed rubrica X)` ⇒ `$version =~
/X/i` exactly per Perl `SetupString::vero` line 299.

## 21. Octave-Day Tempora ranks bake in post-1570 elevations

Tempora files for octave days (`Pent02-5o` Sacred Heart,
`Pent03-0r` post-Octave-of-Sacred-Heart Sunday, etc.) ship a
SINGLE `[Rank]` line that reflects the LATEST status of the
feast — Duplex majus 4.01 for Sacred Heart, etc. Pre-institution
rubrics are expected to suppress these via *officium-string
matching*, not via the data file.

A port has to:
1. Carry a list of post-1570 feast officium-strings (Sacred
   Heart, Patrocinii Joseph, Christi Regis, …).
2. Demote the temporal rank to feria (1.0) when (a) the active
   rubric predates the institution AND (b) the file's officium
   matches one of the post-1570 strings.
3. Skip the file's in-file body sections in proper-block
   resolution under the same gate (mass.rs:is_post_1570_octave_file).

This is opposite to the usual pattern where pre-1570 rubrics
read the bare body and post-1570 ones read a `(sed rubrica X)`
override. For octave days, the *bare* body is the post-1570
form, and pre-1570 rubrics need a programmatic suppression.

## 22. Multiple `[Rank]` formats (post-1955 vs Trident)

Most Sancti files use the standard format:

```
[Rank]
;;Class;;Num;;Commune
(sed rubrica X)
;;ClassX;;NumX;;CommuneX
```

(Single header, multiple bodies separated by `(sed …)` predicates.)

But Sancti/10-07 (Rosary feast) uses the *embedded-name* format:

```
[Rank]
Sanctissimi Rosarii Beatæ Mariæ Virginis;;Duplex 2 classis;;5.1;;ex C11

[Rank1960]
Festum Beatae Mariae Virginis a Rosario;;Duplex 2 classis;;5;;ex C11
```

— the title is *inside* the [Rank] body (first column before
`;;`), and a SEPARATE `[Rank1960]` header (no parenthesised
annotation) carries the post-1960 variant. The embedded-name
form usually replaces a missing `[Officium]` header. A port that
only knows the standard format will mis-bucket these.

## 23. Tempora redirect table (`Tabulae/Tempora/Generale.txt`) is rubric-tagged

Each line in `Generale.txt` has a third column listing the
rubric tokens it applies to:

```
Tempora/Pasc3-0=Tempora/Pasc3-0t;;1888 1906
Tempora/Pasc3-0=Tempora/Pasc3-0r;;1570 1960 Newcal
```

The same `from` stem can have MULTIPLE redirects for different
rubrics. Under T1910 (token `1906`), `Pasc3-0` redirects to
`Pasc3-0t`; under T1570/R55/R60 it redirects to `Pasc3-0r`.

The token in column 3 is the `transfer` column of `Tabulae/data.txt`
keyed by version. NOT the version string itself. Mapping:

| version                  | token  |
|--------------------------|--------|
| Tridentine - 1570        | 1570   |
| Tridentine - 1910        | 1906   |
| Divino Afflatu - 1939    | DA     |
| Reduced - 1955           | 1960   |
| Rubrics 1960 - 1960      | 1960   |
| Monastic Tridentinum 1617| M1617  |

Note: R55's token is `1960` (NOT 1955). And DA has its own
unique `DA` token that doesn't match any line in `Generale.txt`,
so DA-1939 *never* picks up a redirect — every Tempora stem
keeps its bare form.

A port that filters Generale.txt to "1570 lines only" (which
is what the early Phase-7 scaffold did) silently mis-routes
every Tempora redirect on every other rubric.

## 24. `monthday(…, modernstyle, …)` flag is rubric-derived as `$version =~ /196/`

`SetupString.pl:745`:

```perl
$monthday = monthday($day, $month, $year,
                     ($version =~ /196/) + 0, $flag);
```

So `monthday`'s 4th argument flips ONLY for "Rubrics 1960 -
1960" (and "Ordo Praedicatorum - 1962", "Rubrics 1960
Newcalendar"). Pre-1960 rubrics — including R55 — use the
pre-modern week numbering.

Effect on the Sept Embertide week (`093-3 / 093-5 / 093-6`):
- Pre-modern: Embertide is the week after Holy Cross (Sept 14).
- Modern: Embertide is the week of the 3rd Sunday of September.

In a year where the 1st Sunday of September is the 6th or 7th,
the two formulas pick different weeks. Our port flagged this
when it first wired R60 monthday — using `modernstyle=true`
for both R55 and R60 broke R55 by 1 day; restricting to R60
matched Perl.

## 25. `transferred_sancti` heuristic vs. upstream Transfer table

A port that simulates Tridentine transfer rules ("displaced
Duplex+ moves to next free day") to avoid loading the upstream
Transfer table will WRONGLY transfer feasts under post-1570
rubrics. Concrete cases:

- 02-03 (Blasius, Simplex 1.1): under T1910 in 2026, Ignatius
  on 02-01 is preempted by Septuagesima Sunday. The heuristic
  transfers him to 02-03, displacing Blasius. But the upstream
  Transfer table for 2026 (letter d) only has
  `02-03=02-01~02-03;;1570 M1617` — no 1888/1906 entry. So
  T1910 keeps Blasius native.

- 04-17 (Anicetus, Simplex): under T1910 in 2026 with Easter
  Apr 5, Leo I (04-11) is preempted by Easter octave. The
  heuristic transfers him forward to the first free day. The
  upstream table has `04-17=04-11~04-17;;1906` — explicit
  1906/T1910 entry. So the explicit transfer table is needed
  for post-1570 rubrics; the heuristic is a 1570/M1617-only
  approximation.

A clean port gates the heuristic to T1570 and Monastic only,
trusting the explicit Transfer/[letter+easter].txt files for
everything else.

## 26. `(communi Summorum Pontificum)` is a body conditional, not just a section gate

The annotation appears in three places with DIFFERENT semantics:

1. **As a section-header annotation** `[Oratio] (communi Summorum
   Pontificum)`: when SP is active, this body REPLACES the
   default `[Oratio]` body. Same file (Sancti/04-22 SS. Soteris
   et Caji etc.).

2. **As a `(sed communi Summorum Pontificum)` line inside a
   [Rank] block**: when SP is active, the `;;Class;;Num;;Commune`
   line below this annotation REPLACES the default. Mass-side
   (Sancti/03-12 Gregory the Great).

3. **As an inline body conditional** `(sed communi Summorum
   Pontificum)` separating two body lines: when SP is active,
   the line below REPLACES the line above (SCOPE_LINE backscope).
   Used for `in cœlis` → `in cælis` orthographic swap in
   Commune/C4a [Oratio].

A port has to handle ALL THREE call sites. Our Rust uses three
independent dispatches: (1) `annotation_applies_to_rubric` in
`read_section_skipping_annotated`, (2) `_sp_bucket` in
build_missa_json.py emitting `commune_sp`/`rank_num_sp`, and
(3) the `communi` subject branch in `eval_alt_1570`.

Perl's SetupString.pl handles all three uniformly via `vero()` —
the annotation is captured in the conditional regex and
evaluated by the same predicate dispatch table.

## 27. `(nisi communi Summorum Pontificum)` inverts the section gate

Unlike the `(communi Summorum Pontificum)` form which is "use
this body when SP is active", `(nisi communi Summorum
Pontificum)` means "use this body UNLESS SP is active". Pope-
saint files (Sancti/11-23 Clement, Sancti/04-22 Soteris/Caji,
etc.) carry their pre-1942 collect under `(nisi …)` so the body
is shown under T1570/T1910/DA but skipped under R55/R60 (where
the SP commune `vide C2b` body fires instead).

Our parser captures `nisi <X>` annotations the same way as
`<X>` annotations; the runtime annotation evaluator handles
`nisi` by negating the inner predicate. Perl's `vero()` handles
`nisi` natively as part of the conditional grammar.

## 28. `(rubrica 1962)` doesn't apply to "Rubrics 1960 - 1960"

Substring "196" appears in both "1960" and "1962", so a port
that buckets via `"196" in label` over-matches. The Perl
predicate `196` evaluates `$version =~ /196/i` against
"Rubrics 1960 - 1960" and matches; against the LABEL "rubrica
1962" the regex check is `$version =~ /1962/i` which is FALSE.

Our parser's `_post_da_buckets` was originally over-greedy and
treated `(sed rubrica 1962)` as a R60 variant. Sancti/01-24
(Timothy) has `(sed rubrica 1570 aut rubrica monastica aut
rubrica 1962) ;;Simplex;;1.1` — under the broken bucketing,
R60 read Timothy as Simplex 1.1 (wrongly demoted). The 1962
token in upstream is the Dominican OP1962 rubric, not R60.

Fixed by extracting each `19\d{1,3}` token from the annotation
and substring-checking against the version string, matching
Perl's `$version =~ /<token>/` semantic.

## 29. Triduum Mass [Prelude] needs the spelling pipeline too

`mass_propers_from_prelude_only` (the "Full text" branch for
Quad6-5 / Quad6-6) was emitting [Prelude] sub-blocks verbatim
without the spelling layer. Result: under pre-1960 rubrics
"panem nostrum cotidianum" stayed cotidianum (Perl outputs
"quotidianum" via `spell_var`'s `\bco(t[íi]d[íi])` → `quo$1`
substitution).

Lesson: any path that bypasses the regular `proper_block` →
`go` closure flow needs to re-apply the same post-processing
pipeline (macros + spelling + post-Septuagesima conditionals).

## 30. `(deinde dicuntur semper)` opens a forward SCOPE_NEST

Sancti/06-30 [Oratio] body:
```
@:Oratio Pauli
(deinde dicuntur semper)
_
$Oremus
(sed rubrica 196 omittuntur)
@Sancti/01-25:Oratio Petri
```

Under R60, `(sed rubrica 196 omittuntur)` is TRUE; the
`omittuntur` keyword opens a SCOPE_NEST backward, dropping
content back to the previous NEST opener — which is the
preceding `(deinde dicuntur semper)`. So `_$Oremus` get
dropped; `@Sancti/01-25:Oratio Petri` is kept.

Under T1570/DA/R55, the omittuntur predicate is FALSE; no
drop. Result: Pauli + Oremus + Petri.

A port that approximates SCOPE_LINE only (drop one line back)
will mishandle this: under R60 it drops `$Oremus` but keeps
`_`, producing Pauli + `_` + Petri. SCOPE_NEST is essential
for multi-prayer Oratio bodies.

Our Rust currently mishandles this — see RUST_PERL_UNIFICATION_AUDIT.md
for the fix plan. Affects 2 R60 cells (06-30 and the prayer-
concatenation pattern on 10-18 commemorations).

## 31. `[Section](rubrica X)` second-header (no space)

Variant section with a different body for a specific rubric.
Pasc5-4 ships:

```
[Evangelium]
…the long Mark 16:14-20 with pre-1955 Paschal-candle rubric…

[Evangelium](rubrica 1960)
…same Mark 16:14-20 but ends "!Dicto Evangelio exstinguitur
Cereus paschalis." (no "nec ulterius accenditur" trailing)
```

Note no space between `]` and `(`. Different from the
`[Section] (annotation)` form on Rank ([Rank] (rubrica 196 aut
rubrica 1955)) which always has a space. Both forms need to
parse equivalently.

A port has to:
1. Parse the annotation regardless of spacing.
2. Treat the second [Evangelium] as a per-rubric variant
   (similar to [Rank] post-DA second headers but for body
   sections).
3. Evaluate the annotation against the active rubric and prefer
   the variant body when it matches.
4. Cascade through parent inherits — Pasc6-4r inherits from
   Pasc5-4 and should ALSO get the variant under R60 even
   though its own file doesn't carry the variant.

Affects R60 Ascension Mass (Pasc5-4 / Pasc6-4r) plus a handful
of other "rubric-variant body" days.
