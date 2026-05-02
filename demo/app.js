// officium-rs demo — load the WASM, wire up the form, render results.

import init, {
  compute_office_json,
  supported_rubrics,
  version as crateVersion,
} from "./pkg/officium_rs.js";

const $ = (id) => document.getElementById(id);

const COLOR_HEX = {
  White:    "#f5efde",
  Red:      "#a31a2a",
  Violet:   "#4b2a78",
  Black:    "#2a2520",
  Green:    "#3e6b4a",
  Rose:     "#d99a9a",
  Gold:     "#c19c2f",
  // fall-throughs we may emit
  Unknown:  "#999",
};

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));
}

function renderResult(office) {
  if (office.error) {
    return `<div class="status error">Error: ${escapeHtml(office.error)}</div>`;
  }
  const colorHex = COLOR_HEX[office.color] || COLOR_HEX.Unknown;
  const commems = (office.commemorations || [])
    .map((p) => `<li><code>${escapeHtml(p)}</code></li>`)
    .join("");

  return `
    <div class="field">
      <span class="key">winner</span>
      <span class="val"><code>${escapeHtml(office.winner)}</code></span>
    </div>
    <div class="field">
      <span class="key">color</span>
      <span class="val">
        <span class="color-swatch" style="background:${colorHex}"></span>
        ${escapeHtml(office.color)}
      </span>
    </div>
    <div class="field">
      <span class="key">season</span>
      <span class="val">${escapeHtml(office.season)}</span>
    </div>
    <div class="field">
      <span class="key">rank</span>
      <span class="val">${escapeHtml(office.rank) || "<em>—</em>"}</span>
    </div>
    <div class="field">
      <span class="key">rubric</span>
      <span class="val">${escapeHtml(office.rubric)}</span>
    </div>
    <div class="field">
      <span class="key">commemorations</span>
      <span class="val">
        ${commems ? `<ul class="commems">${commems}</ul>` : "<em>none</em>"}
      </span>
    </div>
  `;
}

function setStatus(text, kind = "") {
  const el = $("status");
  el.textContent = text;
  el.className = "status" + (kind ? ` ${kind}` : "");
}

(async function main() {
  // Default the date input to today.
  const today = new Date();
  $("date").value = today.toISOString().slice(0, 10);

  try {
    setStatus("Loading WASM…");
    await init();
    setStatus(`Ready · officium-rs v${crateVersion()}`, "ok");
    $("version").textContent = `officium-rs v${crateVersion()} · WASM`;
    $("submit-btn").disabled = false;
  } catch (err) {
    setStatus(`Failed to load WASM: ${err.message}`, "error");
    return;
  }

  $("query-form").addEventListener("submit", (ev) => {
    ev.preventDefault();
    const dateStr = $("date").value;
    const rubric = $("rubric").value;
    if (!dateStr) return;

    const [y, m, d] = dateStr.split("-").map(Number);

    let office;
    try {
      const json = compute_office_json(y, m, d, rubric);
      office = JSON.parse(json);
    } catch (err) {
      $("result").className = "result";
      $("result").innerHTML = `<div class="status error">Computation failed: ${escapeHtml(err.message)}</div>`;
      return;
    }

    $("result").className = "result";
    $("result").innerHTML = renderResult(office);
  });

  // Auto-compute on load so the user sees something immediately.
  $("query-form").requestSubmit();
})();
