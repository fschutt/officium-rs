// Mass renderer — walks the WASM-emitted `mass.ordinary` list (no
// hardcoded Latin lives in JS) and HTML-formats each entry. The
// shape of `ordinary` is documented in
// `src/wasm.rs::compute_mass_full`.

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
  }[c]));
}

// `!Citation` and `(parens)` lines in the propers text need lighter
// formatting so the dense Latin reads cleanly.
function formatProperBody(latin) {
  if (!latin) return "";
  const lines = latin.split(/\n+/);
  const html = lines
    .map((raw) => {
      const line = raw.trim();
      if (!line) return "";
      if (line.startsWith("!")) {
        return `<div class="citation">${escapeHtml(line.slice(1).trim())}</div>`;
      }
      const vMatch = line.match(/^([VvRrSsMmCcDdJj])\.\s*(.*)$/);
      if (vMatch) {
        return `<div class="vers"><span class="role">${escapeHtml(vMatch[1].toUpperCase())}.</span> ${escapeHtml(vMatch[2])}</div>`;
      }
      if (line === "_") return `<hr class="sep">`;
      if (line.startsWith("(") && line.endsWith(")")) {
        return `<div class="inline-rubric">${escapeHtml(line)}</div>`;
      }
      return `<p>${escapeHtml(line)}</p>`;
    })
    .filter(Boolean)
    .join("\n");
  return html;
}

// Macro bodies (Confiteor / Gloria / Pater noster / DominusVobiscum
// dialog / IteMissaEst variants) come from the Perl Prayers.txt
// multi-line text. Reuse the same role-prefixed line splitter as
// formatProperBody — Prayers.txt uses the same V./R./S. convention.
function formatMacroBody(body) {
  return formatProperBody(body);
}

function renderProper(section, mass) {
  const block = mass.propers[section];
  if (!block) return "";
  const source = block.source
    ? `<div class="proper-source">from <code>${escapeHtml(block.source)}</code>${block.via_commune ? ' <span class="via">via Commune</span>' : ''}</div>`
    : "";
  const body = formatProperBody(block.latin);
  let commems = "";
  if (
    ["oratio", "secreta", "postcommunio"].includes(section) &&
    Array.isArray(mass.propers.commemorations)
  ) {
    for (const c of mass.propers.commemorations) {
      const sub = c[section];
      if (!sub) continue;
      commems += `
        <div class="commem">
          <div class="commem-h">Commemoratio: <code>${escapeHtml(c.source)}</code></div>
          ${formatProperBody(sub.latin)}
        </div>`;
    }
  }
  return `<div class="proper">${source}${body}${commems}</div>`;
}

function renderLine(entry, mass) {
  switch (entry.k) {
    case "section":
      return `<h3 class="ord-heading">${escapeHtml(entry.label)}</h3>`;

    case "rubric":
      return `<div class="rubric" data-level="${entry.level}">${escapeHtml(entry.body)}</div>`;

    case "spoken":
      return `<div class="spoken"><span class="role">${escapeHtml(entry.role)}.</span> ${escapeHtml(entry.body)}</div>`;

    case "plain":
      return `<div class="spoken">${escapeHtml(entry.body)}</div>`;

    case "macro":
      return `<div class="macro" data-macro="${escapeHtml(entry.name)}">${formatMacroBody(entry.body)}</div>`;

    case "proper":
      return renderProper(entry.section, mass);

    case "hook":
      // Side-effect hook fired (Introibo / GloriaM / Credo emitted "omit.").
      return `<div class="rubric">${escapeHtml(entry.message)}</div>`;

    default:
      return "";
  }
}

export function renderMass(mass) {
  const office = mass.office;
  const rules = mass.rules || {};

  const head = `
    <header class="mass-head">
      <h2><code>${escapeHtml(office.winner)}</code></h2>
      <dl class="mass-meta">
        <dt>Rank</dt><dd>${escapeHtml(office.rank || "—")}</dd>
        <dt>Color</dt><dd>${escapeHtml(office.color)}</dd>
        <dt>Season</dt><dd>${escapeHtml(office.season)}</dd>
        <dt>Rubric</dt><dd>${escapeHtml(office.rubric)}</dd>
      </dl>
      <div class="mass-toggles">
        Mode: <strong>${rules.solemn ? "solemn" : "low Mass"}</strong>
        ${rules.defunctorum ? '· <strong>Defunctorum</strong>' : ''}
        · Gloria: <strong>${rules.gloria ? "yes" : "omitted"}</strong>
        · Credo: <strong>${rules.credo ? "yes" : "omitted"}</strong>
        ${rules.prefatio_name ? `· Prefatio: <strong>${escapeHtml(rules.prefatio_name)}</strong>` : ""}
      </div>
    </header>
  `;

  const ordinary = Array.isArray(mass.ordinary) ? mass.ordinary : [];
  const body = ordinary.map((e) => renderLine(e, mass)).filter(Boolean).join("\n");

  return `<article class="mass">${head}${body}</article>`;
}
