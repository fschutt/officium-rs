// officium-rs demo — load the WASM, wire up the form, render the
// full Mass.

import init, {
  compute_mass_json,
  version as crateVersion,
} from "./pkg/officium_rs.js";

import { renderMass } from "./render.js";

const $ = (id) => document.getElementById(id);

function setStatus(text, kind = "") {
  const el = $("status");
  el.textContent = text;
  el.className = kind;
}

(async function main() {
  const today = new Date();
  $("date").value = today.toISOString().slice(0, 10);

  try {
    setStatus("Loading WASM…");
    const t0 = performance.now();
    await init();
    const dt = (performance.now() - t0).toFixed(0);
    setStatus(`Ready · officium-rs v${crateVersion()} · WASM init ${dt} ms`, "ok");
    $("version").textContent = `officium-rs v${crateVersion()} · WASM`;
    $("submit-btn").disabled = false;
  } catch (err) {
    setStatus(`Failed to load WASM: ${err.message}`, "error");
    return;
  }

  function compute(ev) {
    if (ev) ev.preventDefault();
    const dateStr = $("date").value;
    const rubric = $("rubric").value;
    if (!dateStr) return;

    const [y, m, d] = dateStr.split("-").map(Number);

    let mass;
    const t0 = performance.now();
    try {
      const json = compute_mass_json(y, m, d, rubric);
      mass = JSON.parse(json);
    } catch (err) {
      $("result").innerHTML =
        `<div class="error">Computation failed: ${err.message}</div>`;
      return;
    }

    if (mass.error) {
      $("result").innerHTML = `<div class="error">Error: ${mass.error}</div>`;
      return;
    }

    const dt = (performance.now() - t0).toFixed(1);
    $("result").innerHTML = renderMass(mass);
    setStatus(`Rendered ${dateStr} (${rubric}) in ${dt} ms · winner ${mass.office.winner}`, "ok");
  }

  $("query-form").addEventListener("submit", compute);

  // Auto-render today's Mass on load.
  compute();
})();
