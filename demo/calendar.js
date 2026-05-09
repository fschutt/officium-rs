// officium-rs calendar — month-grid view.
//
// For each day in the visible month we call the lightweight
// `compute_calendar_day` WASM export and render the title, color,
// and rank in a simple grid cell. Clicking a cell jumps to the
// Mass / Breviary page for that date.

import init, {
  compute_calendar_day,
  version as crateVersion,
} from "./pkg/officium_rs.js";

const $ = (id) => document.getElementById(id);

const MONTH_NAMES = [
  "", "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];
const DOW_NAMES = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

// Liturgical-color → CSS class.
const COLOR_CLASS = {
  White:  "color-white",
  Red:    "color-red",
  Green:  "color-green",
  Purple: "color-purple",
  Violet: "color-purple", // alias
  Black:  "color-black",
  Rose:   "color-rose",
};

function setStatus(text, kind = "") {
  const el = $("status");
  el.textContent = text;
  el.className = kind;
}

function daysInMonth(year, month) {
  // month: 1-12 (1=Jan)
  return new Date(year, month, 0).getDate();
}

function firstDayOfWeek(year, month) {
  // Returns 0..6 for Sun..Sat, the JS convention.
  return new Date(year, month - 1, 1).getDay();
}

function isToday(year, month, day) {
  const t = new Date();
  return t.getFullYear() === year
      && t.getMonth() + 1 === month
      && t.getDate() === day;
}

function escapeHtml(s) {
  return String(s)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function renderGrid(year, month, rubric) {
  const grid = $("grid");
  grid.innerHTML = "";

  // Day-of-week headers (Sun first).
  for (const dow of DOW_NAMES) {
    const h = document.createElement("div");
    h.className = "dow-head";
    h.textContent = dow;
    grid.appendChild(h);
  }

  // Leading blanks before day 1.
  const firstDow = firstDayOfWeek(year, month);
  for (let i = 0; i < firstDow; i++) {
    const e = document.createElement("div");
    e.className = "cell empty";
    grid.appendChild(e);
  }

  const dim = daysInMonth(year, month);
  for (let d = 1; d <= dim; d++) {
    const cell = document.createElement("a");
    cell.className = "cell";
    cell.href = `./?date=${year}-${String(month).padStart(2, "0")}-${String(d).padStart(2, "0")}&rubric=${encodeURIComponent(rubric)}`;
    cell.title = `Mass for ${MONTH_NAMES[month]} ${d}, ${year}`;
    cell.style.color = "inherit";
    cell.style.textDecoration = "none";
    if (isToday(year, month, d)) cell.classList.add("today");

    let payload;
    try {
      const json = compute_calendar_day(year, month, d, rubric);
      payload = JSON.parse(json);
    } catch (err) {
      payload = { error: err.message };
    }

    if (payload.error) {
      cell.innerHTML = `
        <div class="day-num">${d}</div>
        <div class="day-title" style="color:var(--rubric-red)">${escapeHtml(payload.error)}</div>`;
      grid.appendChild(cell);
      continue;
    }

    const dow = new Date(year, month - 1, d).getDay();
    const colorClass = COLOR_CLASS[payload.color] || "color-green";
    const commem = (payload.commemorations || [])
      .map((c) => c.title)
      .filter(Boolean)
      .join(" · ");

    cell.innerHTML = `
      <div class="day-num">
        <span>${d}</span>
        <span class="dow">${DOW_NAMES[dow]}</span>
      </div>
      <div class="day-title">${escapeHtml(payload.title || payload.winner || "")}</div>
      <div class="day-rank">
        <span class="day-color ${colorClass}" title="${escapeHtml(payload.color || "")}"></span>
        ${escapeHtml(payload.rank || "")}
      </div>
      ${commem ? `<div class="day-commem">${escapeHtml(commem)}</div>` : ""}
    `;
    grid.appendChild(cell);
  }

  $("month-title").textContent = `${MONTH_NAMES[month]} ${year}`;
}

function readState() {
  return {
    year: parseInt($("year").value, 10),
    month: parseInt($("month").value, 10),
    rubric: $("rubric").value,
  };
}

function applyURL() {
  const state = readState();
  const url = new URL(window.location.href);
  url.searchParams.set("year", state.year);
  url.searchParams.set("month", state.month);
  url.searchParams.set("rubric", state.rubric);
  window.history.replaceState({}, "", url.toString());
}

function rerender() {
  const { year, month, rubric } = readState();
  if (!year || !month) return;
  applyURL();
  renderGrid(year, month, rubric);
}

function shiftMonth(delta) {
  const { year, month } = readState();
  let m = month + delta;
  let y = year;
  if (m < 1) { m = 12; y -= 1; }
  if (m > 12) { m = 1; y += 1; }
  $("month").value = String(m);
  $("year").value = String(y);
  rerender();
}

function loadFromQuery() {
  const params = new URL(window.location.href).searchParams;
  const t = new Date();
  const y0 = parseInt(params.get("year"), 10);
  const m0 = parseInt(params.get("month"), 10);
  const r0 = params.get("rubric");
  $("year").value  = String(Number.isFinite(y0) ? y0 : t.getFullYear());
  $("month").value = String(Number.isFinite(m0) && m0 >= 1 && m0 <= 12 ? m0 : t.getMonth() + 1);
  if (r0) {
    const opt = Array.from($("rubric").options).find((o) => o.value === r0);
    if (opt) $("rubric").value = r0;
  }
}

(async function main() {
  loadFromQuery();

  try {
    setStatus("Loading WASM…");
    const t0 = performance.now();
    await init();
    const dt = (performance.now() - t0).toFixed(0);
    setStatus(`Ready · officium-rs v${crateVersion()} · WASM init ${dt} ms`, "ok");
    $("version").textContent = `officium-rs v${crateVersion()} · WASM`;
  } catch (err) {
    setStatus(`Failed to load WASM: ${err.message}`, "error");
    return;
  }

  $("month").addEventListener("change", rerender);
  $("year").addEventListener("change", rerender);
  $("rubric").addEventListener("change", rerender);
  $("prev-btn").addEventListener("click", () => shiftMonth(-1));
  $("next-btn").addEventListener("click", () => shiftMonth(1));
  $("today-btn").addEventListener("click", () => {
    const t = new Date();
    $("year").value = String(t.getFullYear());
    $("month").value = String(t.getMonth() + 1);
    rerender();
  });

  rerender();
})();
