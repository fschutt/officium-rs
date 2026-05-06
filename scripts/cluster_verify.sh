#!/usr/bin/env bash
#
# cluster_verify.sh CLUSTER_NAME
#
# Re-runs the year-sweep on every year referenced by the cluster's
# day list, then checks that each (year, mm-dd) in the cluster file
# is now a passing day in the regenerated manifest.
#
# Workflow:
#   1. read target/regression/clusters/<NAME>.txt
#   2. infer rubric_slug from the file's `# rubric:` header
#   3. group days by year
#   4. for each year, run year-sweep --year YYYY (warm cache makes
#      this ~1-2 s once everything is cached, otherwise cold)
#   5. parse each year's manifest.json — for each cluster day,
#      assert no `differ` / `rust_blank` sections remain
#   6. report PASS / FAIL with a per-day breakdown of any leftovers
#
# Exit code: 0 on full PASS, non-zero on any FAIL.

set -euo pipefail

cluster_name="${1:?usage: $0 CLUSTER_NAME}"
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cluster_file="$project_root/target/regression/clusters/$cluster_name.txt"

if [ ! -f "$cluster_file" ]; then
    echo "ERROR: cluster file missing: $cluster_file" >&2
    echo "       run: scripts/cluster_extract.py $cluster_name" >&2
    exit 2
fi

rubric_slug="$(awk '/^# rubric:/ {print $3; exit}' "$cluster_file")"
desc="$(awk '/^# desc:/ {sub(/^# desc:[[:space:]]+/,""); print; exit}' "$cluster_file")"

# Map slug → rubric-name CLI argument.
case "$rubric_slug" in
    Tridentine_1570)         rubric_arg="Tridentine - 1570" ;;
    Tridentine_1910)         rubric_arg="Tridentine - 1910" ;;
    Divino_Afflatu_1939)     rubric_arg="Divino Afflatu - 1939" ;;
    Reduced_1955)            rubric_arg="Reduced - 1955" ;;
    Rubrics_1960_1960)       rubric_arg="Rubrics 1960 - 1960" ;;
    *) echo "unknown slug: $rubric_slug" >&2; exit 2 ;;
esac

echo "─── verifying cluster: $cluster_name ───"
echo "    rubric: $rubric_arg"
echo "    desc:   $desc"

# Group days by year.
years="$(grep -v '^#' "$cluster_file" | awk -F- '{print $1}' | sort -u)"
total_days=$(grep -cv '^#' "$cluster_file")
echo "    days:   $total_days across $(echo "$years" | wc -l | tr -d ' ') years"
echo

# Build year-sweep release binary if missing.
if [ ! -x "$project_root/target/release/year-sweep" ]; then
    (cd "$project_root" && cargo build --release --bin year-sweep --quiet)
fi

# Run year-sweep on each affected year. Cache hits make these fast.
for year in $years; do
    "$project_root/target/release/year-sweep" \
        --year "$year" \
        --rubric "$rubric_arg" \
        --quiet \
        > /dev/null 2>&1 || true
done

# Verify each cluster day is now passing.
fail_count=0
pass_count=0
fail_lines=""
while IFS= read -r line; do
    [[ "$line" =~ ^# ]] && continue
    [[ -z "$line" ]] && continue
    year="${line%%-*}"
    monthday="${line#*-}"  # MM-DD
    manifest="$project_root/target/regression/$rubric_slug-$year/manifest.json"
    if [ ! -f "$manifest" ]; then
        fail_count=$((fail_count + 1))
        fail_lines+="    $line  (no manifest)"$'\n'
        continue
    fi
    # python one-liner: emit PASS / FAIL for this date based on
    # any `differ` / `rust_blank` sections.
    status=$(python3 - <<PY
import json
with open("$manifest") as f:
    m = json.load(f)
target = "$year-$monthday"
for d in m.get("days", []):
    if d.get("date") == target:
        bad = [s for s in d.get("sections", [])
               if s.get("status") in ("differ", "rust_blank")]
        if bad:
            print("FAIL " + ",".join(s["section"] for s in bad))
        else:
            print("PASS")
        break
else:
    print("MISSING")
PY
    )
    if [[ "$status" == PASS ]]; then
        pass_count=$((pass_count + 1))
    else
        fail_count=$((fail_count + 1))
        fail_lines+="    $line  $status"$'\n'
    fi
done < "$cluster_file"

echo "    PASS: $pass_count"
echo "    FAIL: $fail_count"
if [ -n "$fail_lines" ]; then
    echo
    echo "    failing days:"
    printf "%s" "$fail_lines"
fi
echo

if [ "$fail_count" -gt 0 ]; then
    exit 1
fi
echo "    ✓ cluster $cluster_name fully closed."
