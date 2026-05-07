# Mass-port lessons — final issues to watch out for

**Final state:** Mass renderer is at 100 % Perl-parity across all 5
rubric layers (T1570, T1910, DA, R55, R60) for every day in the
1976-2076 window — 184,455 / 184,455 days passing.

This doc captures the gotchas surfaced during the cluster-closure
loop. Many will recur on the breviary side: the upstream Perl shares
helper modules (`Directorium`, `SetupString`, `DateTime`-like helpers)
with the Mass renderer, and the same data files (Kalendaria,
Transfer/Stransfer tables, Sancti/, Tempora/, Commune/) drive both.

If you're porting the breviary, **read this before designing the
precedence/occurrence layer** — most of these issues live below
`occurrence()`.

---

## 1. Perl `Directorium::transfered()` is substring-regex, not exact-match

`vendor/divinum-officium/web/cgi-bin/DivinumOfficium/Directorium.pm:251`:

```perl
if ($val !~ /^$key/                         # val doesn't start with key
    && (   ($str =~ /$val/i && $val =~ /^$strFolder/)
        || ($val =~ /$str/i))               # val contains str (substring!)
    && $transfer{$key} !~ /v\s*$/i)         # not a `vide-only` rule
```

`$str` is the bare file stem (e.g., `01-28`); `$val` is the rule's
RHS (`01-27~01-28t` after `;;` rubric-flags strip).

**Why it bites:** rule `01-28=01-27~01-28t` — its val mentions
`01-28` only as a *prefix* of `01-28t`. Exact equality on
`target.main` / `extras` misses; Perl's substring regex fires and
suppresses Petri Nolasci (file 01-28 at kalendar 01-31 under 1906
layer) on his native day.

**Rust mirror (`src/transfer_table.rs::stem_transferred_away_with_stems`):**
add a substring path **gated on `source_mmdd == stem`**. Without
the gate, every stem with a suffixed sibling in extras (`02-23o`,
`06-25t`, …) over-fires by ~3000 fails / 28-year sample.

The `^$key` skip Perl uses is mirrored as
`target.main.starts_with(source_mmdd)`. This catches rules like
`06-25=06-25t~…` where the extra "happens to" contain the key.

## 2. Rubric-conditional `[Section] (rubrica X)` — strip annotation when chasing

Sancti files store per-rubric body variants as a literal section
name `"Communio (rubrica 1960)"` (the build script preserves the
suffix rather than collapsing). At the **winner level** the
annotated key is preferred when its predicate matches the active
rubric. But when the body is `@Commune/C6-1` and we chase to that
file, the chased file only carries the **bare** `Communio` key —
the annotation is winner-local.

**Fix:** at the top of `chase_at_reference`, strip a trailing
`(...)` from the `default_section` arg before the chased-file
lookup. Winner-level `read_section` keeps the annotated key; only
cross-file chases see the bare name.

Affects every section type, not just Communio. If the breviary
port has analogous `[Antiphona] (rubrica X)` variants chained
through `@Commune/...`, watch for the same pattern.

## 3. Kalendar key ≠ file stem

Across rubric layers, the kalendar key (the date the office is
served on) and the file stem (the actual `Sancti/MM-DD.txt` file)
diverge. Examples:

* **1906 layer:** kalendar 01-28 → stem `01-28t` (Agnes II);
  kalendar 01-31 → stem `01-28` (Petri Nolasci).
* **1939+ layers:** kalendar 01-28 → stem `01-28` (Petri at his
  natural date); 01-28t becomes a commemoration.

Transfer rules can be keyed on either form. Don't conflate. Our
`occurrence::resolve_sancti_for_tridentine_1570` uses the kalendar
key for transfer-table lookups but iterates sancti entries by file
stem. The reverse-priority easter-file iteration (over the
combined letter+easter file set) lets the easter file's rule
override the letter file's when both apply.

## 4. Layer-aware leap-Feb-23 suppression

In leap years, the bissextile shift puts:

* real Feb 24 → kalendar 02-29 (Vigil of Matthias under 1888/1906);
* Feb 25-28 → kalendar 02-24 onwards (regular saints shifted).

`transfers_for(year, rubric, mm, dd)` consults filter-1
(post-leap-day) on the bare letter + easter files **plus**
filter-2 (pre-leap-day) on `letters[(idx+1)%7]` + (easter+1).
Perl exploits negative indexing (`$letters[$letter-6]`); the Rust
equivalent is `(idx + 1) % 7`.

Filter-1 excludes lines whose LHS is in early-Feb OR whose RHS
starts with `02-2[0123]`. Filter-2 keeps only lines whose LHS is
early-Feb / dirge1.

Skipping this filter breaks every Feb 24-28 in every leap year —
the rule `02-23=02-22~02-23o;;1888 1906` from d.txt would otherwise
re-fire on real Feb 25 in leap year letter c.

## 5. Festum Domini precedence (`horascommon.pl:477-484`)

Class I sancti are **capped at srank 6.01** in Adv/Quad seasons —
a Class I saint does NOT outrank an Adv/Quad Sunday by default.
Festum Domini (e.g., Annunciation 03-25) gets a special exception:
its srank stays high enough to win.

RG 15 (Immaculate Conception 12-08 vs Adv2 Sunday) is a separate
hard-coded exception in `decide_sanctoral_wins_1570`.

If the breviary mirrors precedence by reading [Rank] line + srank,
remember the season-aware cap.

## 6. Suppress Class III feasts under R55 on II classis Sundays

R55 + R60: Class III feasts on a II classis Sunday have their Mass
**Mass-suppressed** (Lauds-only commemoration). This is why
2000-10-22 R55 fails initially: Pent22 outranks Cantius (III
classis), and Cantius gets a Lauds-only commemoration.

Implemented as `apply_r55_simplex_commemoration` in `mass.rs`,
extended to R60 Class II saints (rank ≥ 5) with `_\n$Oremus\n`
separator on the R60 path.

Watch the gate carefully — narrowing it past the WMSunday-key
"104-0" caused Pent19_23_R60 (28 days) to regress when we tried to
broaden it. Stay narrow; expand only after verifying no regression.

## 7. World Mission Sunday (penultimate Sunday of October)

`apply_world_mission_oratio` emits a Propaganda commemoration on
WMSunday. Three rubric branches:

* **R60:** Propaganda gated by transfer-table presence; saint
  commemoration takes Propaganda's slot when saint is Simplex.
* **DA:** multi-Orémus structure (separate `$Oremus` per
  commemoration), parent's `$Per` macro kept. Differs from R55.
* **R55:** sub-unica conclusio — Propaganda inside the parent's
  `$Per`/`$Qui` envelope, single conclusion shared.

If the breviary has analogous WMSunday Vespers / Lauds
commemoration emission, the same three-branch shape probably
applies.

## 8. Multi-line `@-ref` expansion

A section body may be:

```
@:Oratio Pauli
(deinde dicuntur semper)
_
$Oremus
(sed rubrica 196 omittuntur)
@Sancti/01-25:Oratio Petri
```

— composite, with `@`-line resolution per-line. The single-line
chase branch would take only the first line.

Detect via `body.starts_with('@') && body.contains('\n')`; route
to `expand_inline_at_lines`. Single-line `@`-bodies still flow
through the self-reference branch because that's where
`:s/PAT/REPL/[FLAGS]` regex-substitution is parsed.

## 9. `N. ... N.` plural-form replacement

Perl `replaceNdot` replaces the FIRST `N\..*?N\.` (non-greedy,
spans newlines) with the plural form, then any remaining `N.` with
the singular. Order matters — if you replace `N.` singular first,
you'll mangle two-N forms into junk.

`mass.rs::replace_n_dot_plural` mirrors the two-step substitution.

## 10. Sub-unica vs non-sub-unica conclusio

When the winner's `[Rule]` (or its `ex <Path>` chain parents)
carries `Sub unica concl(usione)?`:

* **R60:** strip the **first** `$Per/$Qui` macro line from the
  body. Trailing prayer's terminator stays.
* **Pre-1960:** strip the **final** `$Per/$Qui` macro. Perl saves
  it to `$addconclusio` and re-appends after all commemorations.

R60 ALSO strips when there's a commemoration (no Sub unica
needed). Tempora-winner days commemorating Sancti (Pent21-0
commemorating St Luke 10-18 under R60) hit this path.

`mass.rs::strip_conclusion_macro_for_sub_unica` handles both
branches.

## 11. Christmas Eve match string variants

The string match for "Vigil of the Nativity" must match BOTH:

* `vigilia natalis` (lowercase, some files)
* `Vigilia Nativitatis` (title case, others)

Easy to miss when the regression first surfaces; spot-check with
substring + case-insensitive.

## 12. `(rubrica innovata)` is a non-applies-anywhere annotation

Some files carry `(rubrica innovata)` for a Rank-line variant
(03-06 Perpetua/Felicitas:`;;Duplex;;3.91;;vide C7b`). It's NOT a
predicate dispatch — it just marks the new ranking. Don't try to
match it as a rubric predicate; the body parser skips it as inline
Latin rubric.

## 13. `(sed rubrica 196 omittuntur)` chunk-drop scope

`(sed PRED omittuntur)` is a TRUE-drop conditional. The drop
scope is **CHUNK** (back to the most recent `(deinde X)` or
omittuntur frame), not LINE.

`apply_body_conditionals_1570` tracks a SCOPE_NEST `fence`: the
output offset where the most recent `(deinde X)` opener was set.
On TRUE `(... omittuntur)` truncate output back to fence.

Without this, `Tempora/Quad6-2` [Evangelium] (Holy Week Passion)
and Sancti/06-30 / 01-25 / 02-22 [Oratio]/[Secreta]/[Postcommunio]
(Pope-saint Pauli/Petri commemoration pairs) drop only the last
narrative line instead of the whole chunk.

## 14. Driver / cache invariants

* The persistent Perl driver (`scripts/persistent_driver.pl`) is
  fork-per-render with SHA-keyed disk cache (gzip). When you change
  Perl-side state (vendor submodule update, kalendaria edits),
  invalidate the cache for affected days — otherwise stale
  comparison hits.
* Rayon-parallelism in `year-sweep` is per-year. Within a year,
  days run sequentially (cache hits dominate). Cold cache: ~30 min
  for a full 100-yr × 5-rubric sweep; warm cache: ~90 sec.

## 15. Build-script gotcha — `Commune/C6-1.txt` lives in `horas/`, not `missa/`

`SetupString.pl:547-551` does an automatic basedir swap from `missa`
to `horas` for filenames matching `C\d(?![3-9])[a-z]?` OR when the
missa file doesn't exist but the horas one does. The Mass renderer
silently consumes breviary commune files for several Sancti.

`data/build_missa_json.py` mirrors this fallback. If you find a
Mass section coming up `rust_blank` and Perl renders content, check
whether the chased commune file exists in `missa/Latin/Commune/` —
if not, the build script should pull it from `horas/Latin/Commune/`.

## 16. Reverse priority — easter file > letter file

When both the letter file (e.g., `f.txt`) and the easter-day file
(`331.txt`) ship a rule for the same date, the **easter file wins**.
Iterate entries in reverse order so easter-file rules override
letter-file rules.

`apply_transfer_sancti_1570` does this. Forward iteration was
nuking 03-19 Joseph because letter d's rule fired first.

## 17. Per-rubric rank pick: don't fall through

`rank_num_1570.or(rank_num)` picked pre-1888 rank 3.0 instead of
the R60 rank 6.0 for Joseph 03-20 transfer.

Use `rank_num_1960` first when active rubric is R60, then
`rank_num_1906` for T1910, etc. Don't blindly fall through to a
lower-numbered field.

## 18. T1910 heuristic over-fire guard

`compute_occurrence` for T1910 has a fallback "displaced Vigil OR
leap-year-Feb-24" gate. The heuristic walk that simulates 1570-era
"displaced Duplex+ moves to next free day" only fires when the
date has NO native kalendar entry **OR** the only entry is a
displaced Vigil.

Plain non-leap dates with empty kalendar entries (e.g., 02-12 in
non-1939 layers, where Septem Fundatorum's stem 02-12 lives at
kalendar 02-11) should NOT trigger the heuristic.

## 19. Macro corpus loader — load Prayers.txt early

The macro expander resolves `$Per`, `$Qui`, `$Papa`, `$Oremus`,
etc. from `Prayers.txt` (and friends). Load this in the corpus
trait before invoking the renderer; tests that swap in a
`MockCorpus` need to provide a stub macro store.

`expand_macros` vs `expand_macros_defunctorum` — pick the latter
when the winner's `[Rule]` mentions `defunct` / `c9` / `Add
Defunctorum` (votive of the Dead).

## 20. Ordo template lives in `Ordinarium/` and is rubric-conditional

Mass and Office renderers walk `Ordinarium/<Hour>.txt` (or
`Missa.txt`). These templates have plenty of
`(rubrica X dicitur)` / `(rubrica X omittitur)` /
`$rubrica <Name>` markers. The renderer must respect these for
template content; the regression extractor strips them so they
don't bleed into per-section comparison.

`is_template_rubric_directive` in `regression.rs` enumerates the
shapes — keep it in sync if upstream adds new ones.

---

## Closure-loop fixed-point summary

The 100 % milestone closed on these specific Rust-side fixes
(in commit order):

| Fix | File | What |
|-----|------|------|
| Sub unica concl + R60 commemoration $Per strip | `mass.rs::strip_conclusion_macro_for_sub_unica` | $Per/$Qui macro strip per rubric layer |
| Tempora redirect rubric-conditional | `mass.rs::is_post_1570_octave_file` | Pasc2-5 → fall-through to Sunday |
| Multi-line `@-ref` expansion | `mass.rs::expand_inline_at_lines` | Composite multi-line bodies |
| `(sed rubrica X)` SCOPE_LINE | `mass.rs::apply_body_conditionals_1570` | Per-rubric body conditionals |
| `(sed rubrica X omittuntur)` SCOPE_NEST fence | same | Chunk-drop instead of single-line |
| Festum Domini precedence | `occurrence.rs::decide_sanctoral_wins_1570` | Adv/Quad Class I cap + RG 15 |
| Reverse-priority easter file iteration | `occurrence.rs::apply_transfer_sancti_1570` | Easter file overrides letter |
| Rubric-aware rank pick | same | Pick rank_num for active rubric |
| Layer-aware leap-Feb-23 suppression | same | Filter-1 / filter-2 leap-year split |
| Stem-extras transfer match | `transfer_table.rs::stem_transferred_away_with_stems` | Vigil-of-Matthias real Feb 24 |
| dirge/Hy skip in transfer iteration | same | Skip pseudo-keys |
| Source-key gated Perl substring match | same | Final 01-31 Petri Nolasci closure |
| World Mission Sunday Propaganda gate | `mass.rs::apply_world_mission_oratio` | R60/R55/DA three-branch emission |
| R55 Class III simplex commemoration | `mass.rs::apply_r55_simplex_commemoration` | Sunday + Class III + WMSunday gate |
| `N. ... N.` plural-form replace | `mass.rs::replace_n_dot_plural` | Two-step substitution |
| `[Section] (rubrica X)` rubric-variant lookup | `mass.rs::read_section` | Bare-fall-through + variant-prefer |
| Annotation strip on chase target | `mass.rs::chase_at_reference` | Cross-file chases use bare name |
| T1910 heuristic over-fire guard | `occurrence.rs::compute_occurrence` | Only displaced-Vigil or leap-Feb-24 |
| Driver-cache key includes corpus SHA | `perl_driver.rs` | Cache invalidation across vendor updates |

Closing references: see `docs/CLUSTER_PROGRESS.md` for the full
log; commits `2016770` → `7216ae2` for the final iterations.
