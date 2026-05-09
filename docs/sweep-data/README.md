# 100-year sweep data

`breviary_100y_partial.csv` — paused full-sweep results (1976–1987) of the office_sweep harness across all 5 rubrics × 8 hours, captured after slice 131 (commit `ec11bbf`). One row per (year, rubric).

Columns: `year, rubric, cells, matched, differ, rust_blank, perl_blank, empty, pass_rate`.

Run config:
- Binary: `target/release/office_sweep --year YYYY --hour all --rubric "<NAME>"`
- All 5 rubrics: T1570, T1910, DA, R55, R60.
- Cells per rubric per year: 2920 (non-leap) or 2928 (leap) — that's 365/366 days × 8 hours.
- Sequential, one rubric at a time, on cached Perl driver.

`breviary_100y_partial.log` — per-year wall-clock elapsed.

Years 1976–1987 covered. Years with 0 differs across all 5 rubrics: 1976, 1978, 1981, 1987. The remaining 8 years have residual differs concentrated in DA / R55 / R60 (T1570/T1910 stay near-clean).

Recurring pattern: Compline of `Sat 11-02` cluster (slice 121-style preces-fires-omittitur on the slice 126 swap target) and similar All-Saints-Octave-week residuals. To resume:

```bash
bash /tmp/breviary_100y.sh   # restart from 1988 (edit `seq 1976 2076` start)
```

Run was paused after year 1987 to resume Monday.
