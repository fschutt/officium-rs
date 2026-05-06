# Perl-in-Process — collapse the regression bottleneck

**Status:** plan, not yet implemented.
**Authors:** see git blame.
**Last updated:** 2026-05-06.

## Problem

The Rust↔Perl regression harness shells out to a fresh `perl` subprocess
per (date, rubric, hour). Per call: ~150 ms. The break-down (measured on
`scripts/do_render.sh 04-30-2026 'Tridentine - 1570' SanctaMissa`):

```
~ 50 ms   perl interpreter startup + `use POSIX/CGI/FindBin/locale`
~ 50 ms   require chain (missa.pl → propers.pl → SetupString.pl →
          horascommon.pl → dialogcommon.pl → setup.pl → ordo.pl)
~ 30 ms   setupstring() cold fill — ~hundreds of file opens against
          web/www/horas/Latin/Tempora|Sancti|Commune|Ordinarium|...
~ 20 ms   actual render dispatch + print
```

The first two are constant overhead per process. The third is **the
worst** — `setupstring()` re-walks the upstream corpus from disk every
time, opens hundreds of `.txt` files, parses each line, fills a hash, and
throws the whole thing away when the process exits. DO's I/O pattern is
its single dominant cost.

### Where this fails right now

* **Mass sweep, ±50 yr × 5 rubrics**: 100 yr × 365 day × 5 rubric ×
  150 ms ≈ 7.6 hours wall-clock serial → ~2 hours with the rayon
  parallelism we shipped today, fits the 350-min CI cap with margin.
* **Office sweep, ±50 yr × 5 rubrics × 8 hours**: 8× more renders.
  ~16 hours per matrix job even parallelised. **Doesn't fit the cap.**
  This is the blocker for the breviary regression on `Phase 7-10`.

The disk cache shipped earlier today (keyed on submodule SHA) makes
*reruns* near-free, but the **first** populate of a 100-year × 8-hour
× 5-rubric matrix still has to render every cell once. That populate
is what we need to collapse.

## Approach — Perl-in-process with a memory FS

Make Perl a long-lived in-process component, not a per-render
subprocess, AND give it a memory-resident corpus so `setupstring()`'s
file walk hits RAM instead of the page cache.

### Tier 1 — webperl in wasmer (preferred)

```
┌──────────── regression binary (native Rust) ─────────────┐
│                                                          │
│  rust loop:                                              │
│    for (date, rubric, hour) in dates:                    │
│      send "DATE\tVERSION\tHOUR\n"  ──┐                   │
│      read HTML until SENTINEL       │                    │
│                                     │                    │
│  ┌──────── wasmer runtime ──────────┴──────────────┐     │
│  │                                                 │     │
│  │   webperl.wasm  (Perl 5.28, emscripten build)   │     │
│  │     │                                           │     │
│  │     ├─ STDIN  ← rust pipe                       │     │
│  │     ├─ STDOUT → rust pipe                       │     │
│  │     ├─ memfs at /  (preloaded once)             │     │
│  │     │   ├─ /vendor/divinum-officium/...         │     │
│  │     │   └─ /scripts/persistent_driver.pl        │     │
│  │     └─ argv: ["persistent_driver.pl"]           │     │
│  │                                                 │     │
│  └─────────────────────────────────────────────────┘     │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

`scripts/persistent_driver.pl` is a thin wrapper that:

1. `require`s the missa.pl / officium.pl scripts ONCE, letting their
   top-level `require`s and `setupstring()` populate `%inc` and the
   corpus cache.
2. Loops on stdin: per request, set `@ARGV`, reset the per-render
   globals (`%winner`, `%commemoratio`, `$rule`, etc. — the ones
   missa.pl re-initialises at top of script), redirect STDOUT into a
   buffer, dispatch the render, write `buf + "\x1e\n"` to real STDOUT.
3. Repeats forever.

`\x1e` (RS, ASCII record separator) as the sentinel — never appears in
the rendered Latin/HTML so no escaping needed.

The Rust side keeps the wasmer instance alive for the lifetime of the
sweep and pipes requests serially per worker (we still parallelise
across rubrics by spawning one wasmer instance per rayon thread).

#### Why webperl

* **Self-contained.** No system Perl install needed. `cargo build`
  produces a single binary that runs the regression on any machine
  (including the CI runners and developer laptops). Removes the
  current `apt-get install -y libcgi-pm-perl libdatetime-perl` step.
* **Memory FS.** Emscripten's MEMFS is exactly the in-memory tree
  we want. Files preloaded at instance creation time stay resident
  for the lifetime of the runtime, with no host syscalls per access.
* **Already maintained.** [webperl.zero-g.net](https://webperl.zero-g.net/)
  ships Perl 5.28 + the standard library + DBI / CGI / Time::Local /
  POSIX / FindBin etc. Latest release v0.09 (2020); upstream is
  Hauke Daempfling.
* **No DO patches.** Run upstream missa.pl / officium.pl unmodified;
  the only delta is the persistent driver, which lives in
  `scripts/persistent_driver.pl` (our tree, vendored).

#### Risk register — webperl

* **Wasmer + Emscripten ABI.** Wasmer dropped emscripten support in
  the 4.x branch. Either pin to wasmer 3.x or use an
  emscripten-compatible runtime (e.g. `wasm3`, or
  [`wasmer-emscripten`](https://crates.io/crates/wasmer-emscripten)
  if still published). **First slice = a 50-line spike to confirm
  webperl.wasm runs in some Rust-embeddable runtime.** If this
  doesn't pan out, fall back to Tier 2.
* **WASM bundle size.** webperl.wasm is ~7 MB compressed. Bundling
  it into the regression binary is fine — the binary is a `regression`
  feature-gated dev artifact; doesn't affect the wasm-published lib.
* **State leak across renders.** missa.pl uses ~30 `our $globals` that
  it implicitly resets at top of script. The driver must `local
  ($winner, $commemoratio, ...) = (); { do "missa.pl"; }` per request
  to keep state from bleeding. Risk: missing a global → silent
  cross-contamination. Mitigation: regression test catches it (every
  cached cell needs to match the cache from the subprocess
  baseline).
* **STDOUT capture.** `print` in DO writes to STDOUT. Inside the
  driver loop we need to redirect to a string buffer, then flush.
  Standard Perl idiom: `open(my $fh, '>', \$buf); local *STDOUT = $fh;`.
* **CPAN deps.** missa.pl uses CGI / CGI::Cookie / CGI::Carp /
  Time::Local / POSIX / Encode / FindBin / File::Basename / DateTime.
  webperl ships most of these; **DateTime is the wildcard**. Verify
  in the spike.

### Tier 2 — persistent subprocess + tmpfs (fallback)

If webperl-in-wasmer turns out impractical, the same architecture
falls back to a host-Perl subprocess + tmpfs:

```
┌──────────── regression binary ───────────┐
│  rust loop ←pipes→  $ perl driver.pl     │  long-lived
│                       (working dir =     │   subprocess,
│                        /dev/shm/divinum)  │   per rayon thread
└───────────────────────────────────────────┘
```

* At sweep start: rsync `vendor/divinum-officium/web/www/` to
  `/dev/shm/divinum/web/www/`. ~50 MB; tmpfs is RAM. One-time cost,
  ~1 sec.
* Spawn one persistent perl subprocess per rayon worker, with
  `chdir /dev/shm/divinum`. Same `persistent_driver.pl` protocol as
  Tier 1.
* `setupstring()` walks tmpfs paths — same memcpy speed as MEMFS.
  Process startup happens once per worker, not once per render.

**Pros:** trivially correct (uses the unmodified upstream Perl
binary), works on any Linux with `/dev/shm`, no toolchain risk.
**Cons:** Linux-only (macOS dev machines need
`tmpfs`-equivalent — could shim with a regular dir + `madvise`).
Doesn't remove the system-Perl dependency.

### Tier 3 — current state (baseline)

The on-disk SHA-keyed cache shipped today is **kept** in either
tier — it's the second layer behind the in-process render. Once a
sweep populates it, every later run with the same submodule SHA is
near-free regardless of which renderer we use. The in-process
render only matters for the cold populate.

## Parallel scheduling — instance-per-core pool

The render path is the bottleneck, not aggregation, so the
parallelism model is straightforward: a pool of `N = num_cpus()`
wasmer instances (Tier 1) or persistent subprocesses (Tier 2),
each owned by one rayon worker thread, fed dates from a shared
work queue. The Rust side already has rayon wired in (committed
this morning); the in-process renderer just slots into the
existing `dates.par_iter()` map.

```
                ┌─ rayon worker 0 ─┐  ┌─ rayon worker 1 ─┐  ...
                │                  │  │                  │
                │  PerlDriver {    │  │  PerlDriver {    │
                │    wasmer        │  │    wasmer        │
work queue ───→ │    instance,     │  │    instance,     │  ←── per-thread,
(BTreeMap of    │    memfs (corpus │  │    memfs (corpus │      no contention
 dates)         │      ~50 MB),    │  │      ~50 MB),    │
                │    persistent    │  │    persistent    │
                │      driver.pl   │  │      driver.pl   │
                │  }               │  │  }               │
                └──────────────────┘  └──────────────────┘
```

### Corpus sharing across instances

Each wasmer instance has its own linear memory — no native
sharing — so each pays a corpus-load cost at instance creation.
Approach options, in order of simplicity:

1. **Per-instance MEMFS preload from host.** At instance start,
   read every `vendor/divinum-officium/web/www/**` file off the
   host disk into the instance's MEMFS. Cost: ~50 MB × N
   instances ≈ 400 MB RAM on an 8-core box (CI runners have 7
   GB; fine). Wall-clock: ~1 s per instance, parallel ⇒ ~1 s
   total on first dispatch, then amortised across thousands of
   renders.
2. **Build-time CPIO blob.** `build.rs` walks the vendor tree
   and emits a single CPIO archive embedded via
   `include_bytes!`. Each wasmer instance unpacks it into MEMFS
   at startup. Avoids host-disk reads, makes the sweep
   reproducible bit-for-bit, and removes the
   `vendor/divinum-officium/` runtime dependency for the
   regression binary entirely.
3. **Shared linear memory (WASM threading proposal).** Wasmer
   supports `SharedMemory` for the threading extension, but
   webperl wasn't compiled with `-pthread`, and the Perl
   interpreter isn't reentrant anyway. Not worth the complexity
   — each instance running serially is the right model.

Slice P2/P3 starts with (1) for cheapness; if the build-time
deterministic-blob angle is worth it later, (2) is a one-day
follow-up.

### Per-instance state hygiene

A pool worker reuses the same wasmer instance across many
renders, so the persistent driver MUST reset Perl globals
between requests. missa.pl + officium.pl declare ~30 `our`
variables (`$winner`, `%winner`, `%commemoratio`, `$rule`,
`$duplex`, `$vespera`, `@dayname`, ...) at script top and
implicitly re-init them on each invocation. The driver wraps
each request in a `local`-scoped block:

```perl
# scripts/persistent_driver.pl  (sketch)
require '/vendor/divinum-officium/web/cgi-bin/missa/missa.pl';
# ^ runs once; populates %INC, %setupstring, &dispatch_table

while (my $req = <STDIN>) {
    chomp $req;
    my ($date, $version, $hour) = split /\t/, $req;

    # Reset per-render state. Mirror missa.pl's top-of-script
    # initialisation — list maintained by inspecting upstream
    # `git diff vendor/divinum-officium/web/cgi-bin/missa/missa.pl`.
    local ($winner, %winner, $commemoratio, %commemoratio,
           $rule, $duplex, $vespera, $cvespera, @dayname,
           $rank, $comrank, $litaniaflag, $octavam, ...);

    # Capture render output to a buffer.
    my $buf = '';
    open(my $fh, '>', \$buf) or die "open buf: $!";
    local *STDOUT = $fh;

    @ARGV = ("date=$date", "version=$version", "command=pray$hour");
    do '/vendor/divinum-officium/web/cgi-bin/missa/missa.pl';

    # Restore real stdout, write buf + sentinel.
    close $fh;
    print STDERR "RENDER OK\n" if $@;   # fail-loud on perl error
    syswrite STDOUT, $buf . "\x1e\n";
    STDOUT->flush;
}
```

The `local` block is the discipline that makes the persistent
driver correct under reuse. **Cross-render state leak is
the single biggest correctness risk** — caught by validating
that every cell in the in-process render matches the cached
cell from the per-subprocess baseline (P3 acceptance test).

## Cache integration — in-process render slots BEHIND the disk cache

```
year_sweep / office_sweep
        │
        ▼
render_with_cache(sha, rubric, year, mm, dd, hour, ║)
   ├─ HIT  ─→ read target/regression-cache/<sha>/<rubric>/<YYYY>/<MM-DD>.<hour>.html  (memcpy)
   └─ MISS ─→ ║  fresh-render closure
                ▼
              PerlDriver::render(date, rubric, hour)   ← in-process,
                                                         keeps Perl alive
                ▼
              writes back to disk cache for next time
```

The disk cache shipped this morning **is the artifact**
extracted from a CI sweep. `target/regression-cache/<sha[..12]>/`
is what `actions/cache@v4` saves at end-of-job and restores at
start-of-next-job:

```yaml
- name: Resolve upstream Perl SHA
  id: perl_sha
  run: |
    sha=$(git -C vendor/divinum-officium rev-parse HEAD)
    echo "sha=$sha" >> $GITHUB_OUTPUT

- uses: actions/cache@v4
  with:
    path: target/regression-cache
    key: perl-cache-${{ steps.perl_sha.outputs.sha }}-${{ matrix.rubric }}
```

This is already in `.github/workflows/regression.yml` (commit
`376ac3c`). What this plan adds is faster *cold-cache fill* on the
first run against a new submodule SHA. Once filled, subsequent
runs use the cache regardless of which renderer is configured.

### Cold-fill timing model

| sweep                                          | serial      | rayon × 8 cores | + tier-1/2 | cache hit |
|------------------------------------------------|-------------|-----------------|------------|-----------|
| Mass, 100 yr × 5 rub                          | 7.6 h       | ~1 h            | ~8 min     | ~5 s      |
| Office, 100 yr × 5 rub × 8 hour               | 60+ h       | ~8 h            | **~30 min**| ~30 s     |

The "office × 8 cores serial" row at ~8 h **does not fit** the
350-min CI cap on a single matrix job. Tier-1 / Tier-2 is the
unblock for the office sweep — every other workload survives on
rayon + disk cache alone.

### Sharing the cache across machines

The `target/regression-cache/<sha>/` tree is portable: it's
just files keyed on a deterministic SHA. So:

* CI populates it on first run after a submodule bump → uploads
  via `actions/cache`.
* Developer machines either populate locally OR pull a tarball
  produced by CI (`gh run download <run-id>` → unpack into
  `target/regression-cache/`).
* Dev populate cost: ~30 min once per submodule SHA bump on a
  multi-core laptop; effectively free thereafter.

Worth adding a `scripts/seed-perl-cache.sh` later that pulls the
latest CI cache artifact for the current submodule SHA, so a
fresh checkout doesn't have to re-render. **Tracked as a
follow-up, not in this plan's scope.**

## Concrete protocol — Rust ↔ Perl driver

Stdin requests, one per line, separated by tabs:

```
DATE\tVERSION\tHOUR\n
04-30-2026\tTridentine - 1570\tSanctaMissa\n
```

Stdout responses, one per request, terminated by ASCII RS:

```
<HTML>
...
</HTML>
\x1e\n
```

Rust side: small `PerlDriver` struct that owns the runtime/subprocess
+ a stdin handle + a stdout reader. `PerlDriver::render(date, rubric,
hour) -> Result<String, String>` writes one line, reads until `\x1e`,
returns the buffer.

```rust
pub struct PerlDriver { /* runtime/process handle */ }
impl PerlDriver {
    pub fn new() -> Result<Self, String> { /* spawn, init */ }
    pub fn render(&mut self, date: &str, rubric: &str, hour: &str)
        -> Result<String, String> { /* tab-write, read-until-RS */ }
}
```

Used by `year_sweep` / `office_sweep` exactly where `render_perl`
currently shells out — slot it behind `render_with_cache(...)` so the
disk cache still wraps it.

## Slicing

| ID  | Slice                                                              | Risk | Win                                          |
|-----|--------------------------------------------------------------------|------|----------------------------------------------|
| P1  | webperl.wasm spike: run hello-world from Rust (single instance)    | high | confirms tier-1 viability                    |
| P2  | webperl + DO `require` chain → render one day (single instance)    | high | proves DO loads under webperl                |
| P3  | persistent driver + per-render state reset: 365 days, single rubric| med  | first real timing — target ≤30 s/year        |
| P4  | `PerlDriver` pool — one instance per rayon worker                  | med  | scales with cores; needs state-reset proven  |
| P5  | corpus preload: per-instance MEMFS walk of `vendor/divinum-officium`| low  | ~1 s init × N parallel = ~1 s wall-clock     |
| P6  | wire into year_sweep + office_sweep (slot behind `render_with_cache`)| low  | drop-in; disk cache + CI cache untouched    |
| P7  | CI: bundle webperl.wasm + remove `apt-get install perl` step        | low  | unblocks 8-hour breviary sweep               |
| F1  | tier-2 fallback: persistent subprocess pool + tmpfs                | low  | unblocks immediately if P1/P2 stalls         |
| K1  | (later, optional) build-time CPIO blob via `include_bytes!`        | med  | reproducible cold fill, no host vendor read  |

P1 is the gate. If it fails fast (a few hours of poking at the
runtime), pivot to F1 same day — the user-visible outcome is the
same (regression sweep that fits in CI), the architecture is what
differs.

## Targets

* **Mass-only sweep, 100 yr × 5 rubric**: 7.6 h serial → 8 min
  cold-cache (Tier 1) → 5 sec warm-cache. Already covered by the
  SHA cache + rayon today; this is just a stretch.
* **Office sweep, 100 yr × 5 rubric × 8 hour**: 60+ h serial →
  **target 30 min cold-cache** → 30 sec warm-cache. **This is the
  goal — it unblocks B-leg breviary parity in CI.**

## Acceptance criteria

The in-process renderer is "correct" when, for the same
(date, rubric, hour) tuple, its HTML output is byte-identical to
the per-subprocess baseline. Validation flow during P3:

1. Render N=1000 cells via the per-subprocess path (current
   `do_render.sh`). Save to `/tmp/baseline/`.
2. Render the same N cells via `PerlDriver`. Save to `/tmp/inproc/`.
3. `diff -r /tmp/baseline /tmp/inproc` — must be empty.
4. Run for 10× more cells. Must still be empty.

If step 3 produces a single diff, almost certainly a missed
`local` in the state-reset list. Fail loud, fix, re-run. This
gate must be green before P4 (the pool) goes in — concurrent
state-leak is much harder to debug than serial.

## Out of scope

* Modifying upstream DivinumOfficium. The whole point of webperl /
  tmpfs is that we don't have to touch the vendored Perl. If we
  ever do, it goes in `scripts/divinum-officium.patch` (already a
  pattern in the tree).
* Replacing the Rust port's correctness oracle. The Perl render
  remains the source of truth — we're just running it faster.
* No-std-ifying the regression harness. The harness is native-only
  (`required-features = ["regression"]`) and that's fine.
