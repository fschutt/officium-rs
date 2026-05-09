#!/usr/bin/env bash
# 100-year × 5-rubric office sweep. Writes CSV to /tmp/breviary_100y.csv.
# Clears target/regression/<rubric>-<year>/ output dirs after each year
# (the office_sweep binary doesn't produce them, but year-sweep cache
# from earlier runs sits there — keep target dirs slim).
set -uo pipefail

REPO=/Users/fschutt/Development/officium-rs
OUT=/tmp/breviary_100y.csv
LOG=/tmp/breviary_100y.log
START=$(date +%s)

cd "$REPO" || exit 1

# CSV header
echo "year,rubric,cells,matched,differ,rust_blank,perl_blank,empty,pass_rate" > "$OUT"
> "$LOG"

run_one() {
  local year="$1" rubric="$2"
  local out
  out=$(cargo run --release --bin office_sweep -- \
    --year "$year" --hour all --rubric "$rubric" 2>&1 | tail -10)
  local cells matched differ rblank pblank empty rate
  cells=$(printf '%s\n' "$out" | awk '/^cells:/{print $2; exit}')
  matched=$(printf '%s\n' "$out" | awk '/^matched:/{print $2; exit}')
  differ=$(printf '%s\n' "$out" | awk '/^differ:/{print $2; exit}')
  rblank=$(printf '%s\n' "$out" | awk '/^rust-blank:/{print $2; exit}')
  pblank=$(printf '%s\n' "$out" | awk '/^perl-blank:/{print $2; exit}')
  empty=$(printf '%s\n' "$out" | awk '/^empty:/{print $2; exit}')
  rate=$(printf '%s\n' "$out" | awk '/^pass-rate:/{print $2; exit}')
  printf '%s,"%s",%s,%s,%s,%s,%s,%s,%s\n' \
    "$year" "$rubric" "$cells" "$matched" "$differ" "$rblank" "$pblank" "$empty" "$rate" >> "$OUT"
}

declare -a RUBRICS=(
  "Tridentine - 1570"
  "Tridentine - 1910"
  "Divino Afflatu - 1939"
  "Reduced - 1955"
  "Rubrics 1960 - 1960"
)

for year in $(seq 1976 2076); do
  year_start=$(date +%s)
  for r in "${RUBRICS[@]}"; do
    run_one "$year" "$r"
  done
  # Clear per-year regression output dirs for any rubric (kills accumulated
  # board.html / manifest.json from earlier year-sweep runs).
  rm -rf "$REPO"/target/regression/*-"${year}"/ 2>/dev/null || true
  year_end=$(date +%s)
  elapsed=$((year_end - START))
  yelapsed=$((year_end - year_start))
  printf '[%dm%02ds] year=%d done in %ds\n' \
    $((elapsed / 60)) $((elapsed % 60)) "$year" "$yelapsed" >> "$LOG"
done

end=$(date +%s)
total=$((end - START))
printf '\nTotal: %dm%ds\n' $((total / 60)) $((total % 60)) >> "$LOG"
