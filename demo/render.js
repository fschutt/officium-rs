// Mass renderer — turns the WASM JSON output of compute_mass_json
// into HTML by interleaving the day's propers (introitus, oratio, …)
// with the static Latin Ordinary from `./ordo.js`.

import { ORDO } from "./ordo.js";

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
      // Citation header (`!Heb 13:1`).
      if (line.startsWith("!")) {
        return `<div class="citation">${escapeHtml(line.slice(1).trim())}</div>`;
      }
      // Versicle / responsory / "v." prefix.
      const vMatch = line.match(/^([VvRrSsMmCcDdJj])\.\s*(.*)$/);
      if (vMatch) {
        return `<div class="vers"><span class="role">${escapeHtml(vMatch[1].toUpperCase())}.</span> ${escapeHtml(vMatch[2])}</div>`;
      }
      // Bare separator line.
      if (line === "_") {
        return `<hr class="sep">`;
      }
      // Inline rubric `(Hic genuflectitur)` — Perl renders these in
      // small red italic. The propers we get may still contain them
      // wrapped in parens because apply_body_conditionals_1570 only
      // strips the logical predicates.
      if (line.startsWith("(") && line.endsWith(")")) {
        return `<div class="inline-rubric">${escapeHtml(line)}</div>`;
      }
      return `<p>${escapeHtml(line)}</p>`;
    })
    .filter(Boolean)
    .join("\n");
  return html;
}

function renderEntry(entry, mass, rules) {
  const heading = entry.header
    ? `<h3 class="ord-heading">${escapeHtml(entry.header)}</h3>`
    : "";

  switch (entry.kind) {
    case "rubric":
      return `${heading}<div class="rubric">${escapeHtml(entry.body)}</div>`;

    case "spoken": {
      const role = entry.role
        ? `<span class="role">${escapeHtml(entry.role)}.</span> `
        : "";
      return `${heading}<div class="spoken">${role}${escapeHtml(entry.body)}</div>`;
    }

    case "proper": {
      const block = mass.propers[entry.section];
      if (!block) {
        // Empty proper (Tractus on a non-Lent day, etc.) — silently skip.
        return "";
      }
      const source = block.source
        ? `<div class="proper-source">from <code>${escapeHtml(block.source)}</code>${block.via_commune ? ' <span class="via">via Commune</span>' : ''}</div>`
        : "";
      const body = formatProperBody(block.latin);
      const head = entry.header
        ? `<h3 class="ord-heading proper-h">${escapeHtml(entry.header)}</h3>`
        : "";
      // For Oratio / Secreta / Postcommunio, also render commemorations.
      let commems = "";
      if (
        ["oratio", "secreta", "postcommunio"].includes(entry.section) &&
        Array.isArray(mass.propers.commemorations)
      ) {
        for (const c of mass.propers.commemorations) {
          const sub = c[entry.section];
          if (!sub) continue;
          commems += `
            <div class="commem">
              <div class="commem-h">Commemoratio: <code>${escapeHtml(c.source)}</code></div>
              ${formatProperBody(sub.latin)}
            </div>`;
        }
      }
      return `${head}<div class="proper">${source}${body}${commems}</div>`;
    }

    case "conditional": {
      if (!rules[entry.flag]) return "";
      const inner = entry.entries.map((e) => renderEntry(e, mass, rules)).join("\n");
      return `${heading}<div class="cond">${inner}</div>`;
    }

    case "conditional_branch": {
      for (const branch of entry.branches) {
        if (branch.when_default || rules[branch.when_flag]) {
          return branch.entries
            .map((e) => renderEntry(e, mass, rules))
            .join("\n");
        }
      }
      return "";
    }

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
        Gloria: <strong>${rules.gloria ? "yes" : "omitted"}</strong>
        · Credo: <strong>${rules.credo ? "yes" : "omitted"}</strong>
        ${rules.prefatio_name ? ` · Prefatio: <strong>${escapeHtml(rules.prefatio_name)}</strong>` : ""}
      </div>
    </header>
  `;

  const body = ORDO
    .map((entry) => renderEntry(entry, mass, rules))
    .filter(Boolean)
    .join("\n");

  return `<article class="mass">${head}${body}</article>`;
}
