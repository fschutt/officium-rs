#!/usr/bin/env bash
# Idempotent setup for the vendored Divinum Officium Perl tree at
# vendor/divinum-officium/. Pins the tree to the SHA in
# scripts/divinum-officium.pin, then verifies CPAN deps for do_render.sh.
#
# Refuses to run if the existing vendor tree has uncommitted changes —
# protects ad-hoc developer edits made for debugging the upstream Perl.
#
# See DIVINUM_OFFICIUM_PORT_PLAN.md "Vendoring the Perl reference".

set -eo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

pin_file="scripts/divinum-officium.pin"
vendor_dir="vendor/divinum-officium"
upstream_url="https://github.com/DivinumOfficium/divinum-officium.git"

err() { printf '\033[31mFATAL:\033[0m %s\n' "$1" >&2; }
ok()  { printf '\033[32mOK\033[0m %s\n' "$1"; }
note(){ printf '   %s\n' "$1"; }

# --- 1. Read pin --------------------------------------------------------
if [ ! -f "$pin_file" ]; then
  err "$pin_file missing"
  exit 1
fi
pin_sha="$(awk 'NR==1{print $1; exit}' "$pin_file" | tr -d '[:space:]')"
if [ -z "$pin_sha" ]; then
  err "empty pin in $pin_file"
  exit 1
fi
short_sha="$(printf '%s' "$pin_sha" | cut -c1-10)"

# --- 2. Clone or update -------------------------------------------------
if [ ! -d "$vendor_dir/.git" ]; then
  echo "Cloning $upstream_url into $vendor_dir..."
  git clone --quiet "$upstream_url" "$vendor_dir"
else
  ( cd "$vendor_dir"
    if ! git diff --quiet || ! git diff --cached --quiet; then
      err "$vendor_dir has uncommitted changes; refusing to clobber"
      note "stash or commit them, or rm -rf $vendor_dir to start fresh"
      exit 1
    fi
    git fetch --quiet origin
  )
fi

# --- 3. Pin -------------------------------------------------------------
( cd "$vendor_dir"
  current_sha="$(git rev-parse HEAD)"
  if [ "$current_sha" != "$pin_sha" ]; then
    echo "Pinning to $short_sha..."
    git checkout --quiet "$pin_sha"
  fi
)

# --- 4. Smoke check: render entrypoint exists ---------------------------
missa_pl="$vendor_dir/web/cgi-bin/missa/missa.pl"
officium_pl="$vendor_dir/web/cgi-bin/horas/officium.pl"
if [ ! -f "$missa_pl" ] || [ ! -f "$officium_pl" ]; then
  err "render entrypoints missing in vendor — clone may be incomplete"
  exit 1
fi

# --- 5. Verify CPAN deps ------------------------------------------------
# Runtime deps for missa.pl / officium.pl, drawn from `use` lines in those
# scripts and Build.pl. Ordered by likelihood of being missing.
deps="CGI CGI::Cookie CGI::Carp DateTime File::Basename Time::Local POSIX FindBin Encode"
missing=""
for dep in $deps; do
  if ! perl -e "use $dep; 1" 2>/dev/null; then
    missing="$missing $dep"
  fi
done
if [ -n "$missing" ]; then
  err "missing Perl modules:$missing"
  echo
  echo "Install with cpanm (preferred):" >&2
  echo "  cpanm$missing" >&2
  echo "or via cpan:" >&2
  echo "  cpan$missing" >&2
  echo
  echo "If cpanm itself is missing:" >&2
  echo "  curl -L https://cpanmin.us | perl - --sudo App::cpanminus" >&2
  exit 1
fi

# --- 6. Done ------------------------------------------------------------
ok "vendor/divinum-officium pinned at $short_sha"
ok "Perl deps present ($(echo "$deps" | wc -w | tr -d ' ') modules)"
echo
echo "Smoke test:"
echo "  bash scripts/do_render.sh 04-30-2026 'Tridentine - 1570' SanctaMissa | grep -i Introitus"
