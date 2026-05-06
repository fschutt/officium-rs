#!/usr/bin/env perl
#
# persistent_driver.pl — long-lived Perl process for the regression
# harness. Loads upstream missa.pl / officium.pl ONCE so the
# require-chain + setupstring corpus walk only happens at startup;
# subsequent renders only pay for the actual render dispatch
# (~20 ms vs ~150 ms for a fresh subprocess).
#
# Protocol:
#   stdin  — one request per line, three TAB-separated fields:
#               DATE\tVERSION\tHOUR\n
#               05-04-2026\tTridentine - 1570\tSanctaMissa\n
#            Special line "QUIT\n" exits cleanly.
#   stdout — length-prefixed render output:
#               LEN <bytes>\n
#               <bytes of UTF-8 HTML>
#            No trailing newline after the body so the byte-count
#            matches exactly. The Rust caller reads the LEN line,
#            then reads exactly that many bytes.
#   stderr — diagnostic prefix lines: "READY", "ERROR ...",
#            "WARMUP ...". The Rust caller uses "READY" as the
#            handshake signal.
#
# Usage:
#   perl persistent_driver.pl [VENDOR_ROOT [MISSA_PL [OFFICIUM_PL]]]
#
#   VENDOR_ROOT   — path to vendor/divinum-officium (default: derived
#                   from $FindBin::Bin)
#   MISSA_PL      — path to missa/missa.pl
#   OFFICIUM_PL   — path to horas/officium.pl
#
# Correctness model:
#   `do FILE` re-evaluates the entrypoint script per request, but
#   the upstream Perl has many caches scattered across modules
#   (`_dialog{loaded}`, vespera-line counters, `$datafolder`, ...)
#   that aren't reset by re-running the top of missa.pl /
#   officium.pl. Enumerating them all is fragile.
#
#   Instead, we fork per render: the parent pre-loads every Perl
#   module and primes the setupstring corpus cache once at startup
#   (the warmup render below), then loops on stdin and forks a
#   child for each render request. The child inherits all loaded
#   state via copy-on-write — including the corpus cache — so it
#   doesn't pay the cold-fill cost. The child does ONE render,
#   writes the framed output directly to STDOUT, and exits. State
#   leaks between renders are impossible because each child has
#   its own address space.
#
#   Linux fork is ~3-5 ms; render itself is ~20-30 ms. Net per
#   request: ~25-35 ms (~5× faster than a fresh subprocess).

use strict;
use warnings;
use FindBin;
use File::Basename qw(dirname);
# Load CGI eagerly so we can call CGI::initialize_globals() before
# the first render too. Both missa.pl and officium.pl `use CGI;`
# themselves; loading it here just hoists that into our process
# init phase rather than the warmup render.
use CGI;

my $script_type = $ARGV[0] || 'missa';   # 'missa' or 'officium'
my $vendor_root = $ARGV[1] || "$FindBin::Bin/../vendor/divinum-officium";
my $missa_pl    = $ARGV[2] || "$vendor_root/web/cgi-bin/missa/missa.pl";
my $officium_pl = $ARGV[3] || "$vendor_root/web/cgi-bin/horas/officium.pl";

if ($script_type ne 'missa' && $script_type ne 'officium') {
    print STDERR "ERROR: script_type must be 'missa' or 'officium', got '$script_type'\n";
    exit 2;
}

# Pre-resolve the per-script $Bin values we'll temporarily install
# during `do FILE`. missa.pl / officium.pl use `FindBin::$Bin` for
# both `use lib` and `require` paths, but FindBin sets `$Bin` ONCE
# at process startup based on `$0`, which here is the persistent
# driver — not the entrypoint file. We swap it back per render.
my $missa_bin    = dirname($missa_pl);
my $officium_bin = dirname($officium_pl);

unless (-e $missa_pl && -e $officium_pl) {
    print STDERR "ERROR vendor scripts missing:\n";
    print STDERR "  missa:    $missa_pl (",    (-e $missa_pl    ? "ok" : "missing"), ")\n";
    print STDERR "  officium: $officium_pl (", (-e $officium_pl ? "ok" : "missing"), ")\n";
    exit 2;
}

# Make the vendor lib path visible the same way `perl missa.pl` would
# (missa.pl uses `use lib "$Bin/.."` to pull in DivinumOfficium::* modules).
unshift @INC, "$vendor_root/web/cgi-bin/missa";
unshift @INC, "$vendor_root/web/cgi-bin/horas";
unshift @INC, "$vendor_root/web/cgi-bin";

# Disable buffering on STDOUT/STDERR so the Rust caller sees each
# render the moment the perl side finishes writing it.
$| = 1;
{ my $prev = select STDERR; $| = 1; select $prev; }

# STDOUT must be in raw mode for length-prefix framing — the byte
# count on the LEN line refers to UTF-8 bytes on the wire, not
# characters. We open per-render buffers as `>:encoding(utf-8)`
# so $buf already holds UTF-8 bytes; just need STDOUT itself in
# binary mode.
binmode STDOUT;

# Render one (date, version, hour) request. Returns the buffered
# HTML and an optional error string. Captures STDOUT into a string
# scalar via `local *STDOUT`; the dynamic-scope assignment
# propagates into all subroutines `do FILE` ends up calling.
sub render_one {
    my ($date, $version, $hour) = @_;

    my $script  = $hour eq 'SanctaMissa' ? $missa_pl : $officium_pl;
    my $bin     = $hour eq 'SanctaMissa' ? $missa_bin : $officium_bin;
    my $command = "pray$hour";

    # Mirror do_render.sh's argv shape — missa.pl/officium.pl read
    # `key=value` tokens from @ARGV when not invoked as a CGI.
    local @ARGV = (
        "version=$version",
        "command=$command",
        "date=$date",
    );

    # Temporarily point FindBin at the entrypoint's own directory
    # so that `use lib "$Bin/.."` and `require "$Bin/../..."` lines
    # inside missa.pl / officium.pl resolve to the upstream
    # DivinumOfficium tree, not to our scripts/ directory.
    local $FindBin::Bin       = $bin;
    local $FindBin::RealBin   = $bin;
    local $FindBin::Script    = ($hour eq 'SanctaMissa') ? 'missa.pl' : 'officium.pl';
    local $FindBin::RealScript = $FindBin::Script;
    local $0                  = $script;

    # CGI parses @ARGV ONCE per process and caches the result. In
    # the child this happens fresh, but the parent process kept
    # the warmup date's params; force a reset before fork so the
    # child sees a clean slate even if it skips reset-on-init.
    if (CGI->can('initialize_globals')) {
        CGI::initialize_globals();
    }

    # dialogcommon.pl's `_dialog` cache picks `missa.dialog` or
    # `horas.dialog` based on `$datafolder` at first access, then
    # guards reload with `$_dialog{'loaded'}`. Reset both flag and
    # cache so this render pulls the right dialog file for its
    # script type.
    %main::_dialog = ();

    # Request-input package globals that the entrypoint scripts
    # READ but don't always RE-INITIALISE before reading. The
    # parent's warmup populated these; without resetting, the
    # child's `do $script` short-circuits any "if undef → read
    # CGI param" fallback. Notably:
    #   $date1        — `precedence($date1)` is called with the
    #                   stale parent value, which truthifies past
    #                   the `strictparam('date')` fallback in
    #                   `horascommon.pl:1549`.
    #   $browsertime  — `gettoday()` returns it if truthy.
    #   $command      — used in `if ($command =~ /next/i) { ... }`.
    #   @horas, $hora — driven by `$command` decoder.
    #   $completed    — read from cookies; reset to be safe.
    $main::date1       = undef;
    $main::browsertime = undef;
    $main::command     = undef;
    $main::caller      = undef;
    $main::completed   = undef;
    $main::hora        = undef;
    @main::horas       = ();
    @main::dayname     = ();
    # Per-render counters in webdia.pl that build HTML element IDs
    # (`Vespera1`, `Vespera2`, ...). Without resetting, the warmup
    # leaves the counter at 54 and the next render emits
    # `Vespera55`, `Vespera56`, ... — 25-byte diff per call.
    $main::searchind   = 0;
    $main::dId         = 0;

    # Capture STDOUT into a scalar. `:encoding(utf-8)` matches the
    # `binmode(STDOUT, ':encoding(utf-8)')` that missa.pl sets at
    # top-of-script — without it the rendered Latin double-encodes.
    my $buf = '';
    open(my $fh, '>:encoding(utf-8)', \$buf)
        or return ('', "open buffer: $!");

    my $err;
    {
        local *STDOUT = $fh;
        # `do FILE` re-evaluates the entrypoint. Globals like
        # $winner / %winner / $rule / $duplex / $vespera / @dayname
        # get re-initialised by the entrypoint's own top-level
        # statements — same reset the subprocess baseline gets.
        # `require` lines inside the entrypoint no-op on the second
        # call (Perl caches via %INC), so we don't pay the
        # module-load cost again.
        my $rc = do $script;
        if (!defined $rc) {
            $err = "do $script: $@";
        } elsif ($@) {
            $err = "eval $script: $@";
        }
    }
    close $fh;

    # Mirror `grep -v '^Set-Cookie:'` from do_render.sh — CGI module
    # may emit Set-Cookie headers that the regression harness
    # doesn't want in the body.
    $buf =~ s/^Set-Cookie:[^\n]*\n//mg;

    # `htmlEnd()` prints `</FORM></BODY></HTML>` without a trailing
    # newline. The subprocess pipeline (`perl | grep -v Set-Cookie`)
    # ends up with a trailing newline because grep emits one for
    # the final line. Match that for byte-for-byte parity.
    $buf .= "\n" unless $buf =~ /\n\z/;

    return ($buf, $err);
}

# ─── Pre-warm ───────────────────────────────────────────────────────
#
# Render ONE date for the script-type this driver instance owns.
# We deliberately do NOT pre-warm both missa.pl AND officium.pl in
# the same process: missa/ordo.pl and horas/horas.pl both define
# `sub getordinarium` in `package main`, so loading both leaves
# whichever was loaded last bound to that name globally — and
# `require` is one-shot, so a later `do FILE` won't re-bind. Each
# driver process therefore handles ONE script type; the Rust
# caller spawns two drivers (one per type) when it needs both.
{
    my ($warm_date, $warm_hour) =
        $script_type eq 'missa'
            ? ('05-04-2026', 'SanctaMissa')
            : ('05-04-2026', 'Vespera');
    my ($buf, $err) = render_one($warm_date, 'Tridentine - 1570', $warm_hour);
    print STDERR "WARMUP $script_type FAILED: $err\n" if defined $err;
}

# Tell the Rust caller we're ready to service requests.
print STDERR "READY\n";

# ─── Main loop ──────────────────────────────────────────────────────
#
# Per request: fork a child. Child has the parent's full warmed
# state via CoW. Child renders, writes framed output to STDOUT,
# exits. Parent waits for the child, then reads the next request.
while (my $line = <STDIN>) {
    chomp $line;
    last if $line eq 'QUIT';

    my ($date, $version, $hour) = split /\t/, $line, 3;
    if (!defined $date || !defined $version || !defined $hour) {
        print STDERR "MALFORMED REQUEST: $line\n";
        print "LEN 0\n";
        STDOUT->flush;
        next;
    }

    # Reject hours this driver wasn't built to handle. The Rust
    # caller is supposed to route to the right driver instance;
    # surfacing a clear error here makes routing bugs easy to spot.
    my $is_missa_hour = $hour eq 'SanctaMissa';
    if ($script_type eq 'missa' && !$is_missa_hour) {
        print STDERR "ROUTING ERROR: missa-driver got hour=$hour\n";
        print "LEN 0\n";
        STDOUT->flush;
        next;
    }
    if ($script_type eq 'officium' && $is_missa_hour) {
        print STDERR "ROUTING ERROR: officium-driver got hour=$hour\n";
        print "LEN 0\n";
        STDOUT->flush;
        next;
    }

    my $pid = fork();
    if (!defined $pid) {
        print STDERR "FORK FAILED: $!\n";
        print "LEN 0\n";
        STDOUT->flush;
        next;
    }

    if ($pid == 0) {
        # ─── CHILD: do the render and write framed output ───
        my ($html, $err) = render_one($date, $version, $hour);
        if (defined $err) {
            print STDERR "ERROR rendering $date | $version | $hour: $err\n";
        }
        use bytes;
        my $len = length $html;
        no bytes;
        # Write the framed response. We use syswrite to avoid PerlIO
        # buffering layers re-entering the (now stale) parent's STDOUT
        # handle state — children get a fresh syswrite into the inherited
        # fd directly.
        syswrite STDOUT, "LEN $len\n";
        syswrite STDOUT, $html;
        # Avoid global destructors that might re-emit content from the
        # warmed parent state — exec out via POSIX::_exit so DESTROY
        # blocks on objects we inherited don't run.
        require POSIX;
        POSIX::_exit(0);
    }

    # ─── PARENT: wait for the child to finish writing + exit ───
    waitpid($pid, 0);
    if ($? != 0) {
        print STDERR "CHILD exited with status $?\n";
    }
}
