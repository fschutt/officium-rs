#!/usr/bin/env bash
# do_render.sh — invoke the upstream Divinum Officium Perl render
# entrypoint as a CLI script. The Perl is the regression oracle for the
# Rust port; this wrapper hides the CGI-as-script invocation shape.
#
# Usage:
#   do_render.sh DATE VERSION HOUR
#
#   DATE     MM-DD-YYYY (US format — what the Perl expects)
#   VERSION  one of:
#              "Tridentine - 1570"
#              "Tridentine - 1910"
#              "Divino Afflatu"
#              "Reduced - 1955"
#              "Rubrics 1960 - 1960"
#              "pre-Trident Monastic"
#   HOUR     SanctaMissa | Matutinum | Laudes | Prima | Tertia |
#            Sexta | Nona | Vespera | Completorium
#
# Emits HTML on stdout; Set-Cookie lines stripped. Exit non-zero if the
# vendor tree is missing — points at scripts/setup-divinum-officium.sh.

set -eo pipefail

if [ "$#" -ne 3 ]; then
  cat <<EOF >&2
Usage: $0 DATE VERSION HOUR
  e.g.: $0 04-30-2026 'Tridentine - 1570' SanctaMissa
EOF
  exit 2
fi

date="$1"
version="$2"
hour="$3"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
vendor_dir="$repo_root/vendor/divinum-officium"

if [ ! -d "$vendor_dir/web/cgi-bin" ]; then
  cat <<EOF >&2
FATAL: $vendor_dir missing.
       Run: bash scripts/setup-divinum-officium.sh
EOF
  exit 1
fi

# Map hour to entrypoint script + command. Mass uses missa/missa.pl;
# every Office hour uses horas/officium.pl. Command is "pray<HOUR>".
case "$hour" in
  SanctaMissa)
    script="$vendor_dir/web/cgi-bin/missa/missa.pl"
    ;;
  Matutinum|Laudes|Prima|Tertia|Sexta|Nona|Vespera|Completorium)
    script="$vendor_dir/web/cgi-bin/horas/officium.pl"
    ;;
  *)
    echo "FATAL: unknown HOUR '$hour'" >&2
    echo "       valid: SanctaMissa Matutinum Laudes Prima Tertia Sexta Nona Vespera Completorium" >&2
    exit 2
    ;;
esac
command="pray$hour"

# The Perl reads CGI params from @ARGV when not invoked via a web server.
# Passing them as "key=value" tokens is the convention used by the
# upstream regression script (regress/scripts/generate-diff.sh).
exec perl "$script" \
  "version=$version" \
  "command=$command" \
  "date=$date" \
| grep -v '^Set-Cookie:'
