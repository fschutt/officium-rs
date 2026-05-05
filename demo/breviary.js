// officium-rs Breviary demo — load the WASM, wire up the form,
// render one Hour of the Divine Office.
//
// The line shape returned by `compute_office_full` is identical to
// the `mass.ordinary` array — `{k, body, label, role, level, name}`
// — so the same renderer fragment from `render.js` could in
// principle be reused. We inline a slimmer renderer here because
// the office has no proper-block JSON to splice (B7 minimum scope:
// Section + Plain + Macro + Rubric + Spoken). When B7+ adds
// proper blocks, share the renderer with `render.js`.

import init, {
  compute_office_full,
  version as crateVersion,
} from "./pkg/officium_rs.js";

const $ = (id) => document.getElementById(id);

function setStatus(text, kind = "") {
  const el = $("status");
  el.textContent = text;
  el.className = kind;
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
  }[c]));
}

// Lightly format a multi-line body — handles `!Citation`, `V./R.`
// role-prefixed lines, `(parens)` rubric annotations, and the `_`
// paragraph separator. Same rules as render.js::formatProperBody.
function formatBody(latin) {
  if (!latin) return "";
  return latin
    .split(/\n+/)
    .map((raw) => {
      const line = raw.trim();
      if (!line) return "";
      if (line.startsWith("!")) {
        return `<div class="citation">${escapeHtml(line.slice(1).trim())}</div>`;
      }
      const vMatch = line.match(/^([VvRrSsMmCcDdJj])\.\s*(.*)$/);
      if (vMatch) {
        return `<div class="vers"><span class="role">${escapeHtml(
          vMatch[1].toUpperCase()
        )}.</span> ${escapeHtml(vMatch[2])}</div>`;
      }
      if (line === "_") return `<hr class="sep">`;
      if (line.startsWith("(") && line.endsWith(")")) {
        return `<div class="inline-rubric">${escapeHtml(line)}</div>`;
      }
      return `<p>${escapeHtml(line)}</p>`;
    })
    .filter(Boolean)
    .join("\n");
}

function renderLine(entry) {
  switch (entry.k) {
    case "section":
      return `<h3 class="ord-heading">${escapeHtml(entry.label)}</h3>`;
    case "rubric":
      return `<div class="rubric" data-level="${entry.level}">${escapeHtml(
        entry.body
      )}</div>`;
    case "spoken":
      return `<div class="spoken"><span class="role">${escapeHtml(
        entry.role
      )}.</span> ${escapeHtml(entry.body)}</div>`;
    case "plain":
      return formatBody(entry.body);
    case "macro":
      return `<div class="macro" data-macro="${escapeHtml(entry.name)}">${formatBody(
        entry.body
      )}</div>`;
    case "proper":
      // For B7 the office walker emits `proper` markers as forward
      // references to slot fillers we haven't wired yet (B8+).
      // Render them as a faint placeholder so the structure is
      // visible without claiming text we haven't resolved.
      return `<div class="rubric">[${escapeHtml(entry.section)}]</div>`;
    case "hook":
      return `<div class="rubric">${escapeHtml(entry.message || entry.hook)}</div>`;
    default:
      return "";
  }
}

function renderOffice(payload) {
  const office = payload.office || {};
  const lines = Array.isArray(payload.lines) ? payload.lines : [];

  const head = `
    <header class="office-head">
      <h2><code>${escapeHtml(office.day_key || "—")}</code></h2>
      <dl class="office-meta">
        <dt>Hour</dt><dd>${escapeHtml(office.hour || "—")}</dd>
        <dt>Rubric</dt><dd>${escapeHtml(office.rubric || "—")}</dd>
        <dt>First Vespers</dt><dd>${office.first_vespers ? "yes" : "no"}</dd>
      </dl>
    </header>
  `;

  const body = lines.map(renderLine).filter(Boolean).join("\n");
  return `<article class="office">${head}${body}</article>`;
}

(async function main() {
  // Default to St. Monica (May 4) — matches the existing test
  // anchor in horas.rs so users see real data immediately.
  if (!$("date").value) {
    $("date").value = "2026-05-04";
  }

  try {
    setStatus("Loading WASM…");
    const t0 = performance.now();
    await init();
    const dt = (performance.now() - t0).toFixed(0);
    setStatus(
      `Ready · officium-rs v${crateVersion()} · WASM init ${dt} ms`,
      "ok"
    );
    $("version").textContent = `officium-rs v${crateVersion()} · WASM`;
    $("submit-btn").disabled = false;
  } catch (err) {
    setStatus(`Failed to load WASM: ${err.message}`, "error");
    return;
  }

  function compute(ev) {
    if (ev) ev.preventDefault();
    const dateStr = $("date").value;
    if (!dateStr) return;
    const [y, m, d] = dateStr.split("-").map(Number);
    const rubric = $("rubric").value;
    const hour = $("hour").value;
    const dayKey = $("day-key").value.trim();
    const nextDayKey = $("next-day-key").value.trim();
    const rubricsOn = $("rubrics-on").checked;

    if (!dayKey) {
      $("result").innerHTML = `<div class="error">Day key is required.</div>`;
      return;
    }

    let payload;
    const t0 = performance.now();
    try {
      const json = compute_office_full(
        y,
        m,
        d,
        rubric,
        hour,
        dayKey,
        nextDayKey,
        rubricsOn
      );
      payload = JSON.parse(json);
    } catch (err) {
      $("result").innerHTML = `<div class="error">Computation failed: ${err.message}</div>`;
      return;
    }
    if (payload.error) {
      $("result").innerHTML = `<div class="error">Error: ${escapeHtml(payload.error)}</div>`;
      return;
    }

    const dt = (performance.now() - t0).toFixed(1);
    $("result").innerHTML = renderOffice(payload);
    setStatus(
      `Rendered ${dateStr} ${hour} (${rubric}) in ${dt} ms · ${payload.office.day_key}${payload.office.first_vespers ? " · first Vespers" : ""}`,
      "ok"
    );
  }

  $("query-form").addEventListener("submit", compute);

  // Auto-render on load.
  compute();
})();
