#!/usr/bin/env bash
# 100-year × 5-rubric office sweep. Appends to docs/sweep-data CSV.
# Runs cargo clean after each year so target/ doesn't grow unbounded
# (perl HTML cache + regression scratch can hit GB scale otherwise,
# and a full disk silently breaks cargo's next build). The cost is
# a ~60s rebuild at the top of each year's first cargo run.
#
# Always uses --release; no debug/dev profile artifacts are written.
#
# Resumable: edit START_YEAR / END_YEAR to slice the range. Appends to the
# CSV/log, never overwrites — header is only written if the CSV does not
# already exist.
set -uo pipefail

REPO=/Users/fschutt/Development/officium-rs
OUT="$REPO/docs/sweep-data/breviary_100y_partial.csv"
LOG="$REPO/docs/sweep-data/breviary_100y_partial.log"
START_YEAR=${START_YEAR:-2011}
END_YEAR=${END_YEAR:-2076}
START=$(date +%s)

cd "$REPO" || exit 1

# Only write header if CSV is missing.
if [[ ! -f "$OUT" ]]; then
  echo "year,rubric,cells,matched,differ,rust_blank,perl_blank,empty,pass_rate" > "$OUT"
fi
touch "$LOG"

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

for year in $(seq "$START_YEAR" "$END_YEAR"); do
  year_start=$(date +%s)
  for r in "${RUBRICS[@]}"; do
    run_one "$year" "$r"
  done
  # Strip per-year regression output dirs first (cheap), then full
  # cargo clean to drop the unbounded perl HTML cache + release
  # artifacts. Next year's first run rebuilds the binary in ~60s.
  rm -rf "$REPO"/target/regression/*-"${year}"/ 2>/dev/null || true
  cargo clean -q 2>>"$LOG" || true
  year_end=$(date +%s)
  elapsed=$((year_end - START))
  yelapsed=$((year_end - year_start))
  printf '[%dm%02ds] year=%d done in %ds\n' \
    $((elapsed / 60)) $((elapsed % 60)) "$year" "$yelapsed" >> "$LOG"
done

end=$(date +%s)
total=$((end - START))
printf '\nRange %d-%d total: %dm%ds\n' \
  "$START_YEAR" "$END_YEAR" $((total / 60)) $((total % 60)) >> "$LOG"
