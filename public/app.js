// street-smarts v0.1 — browser entrypoint
// Loads WASM, fetches Eastside Commons fixtures, renders the disagreement card.

import init, { evaluate_neighborhood, library_info } from './wasm/street_smarts_web.js';

const FIXTURES = {
  baseline: './eastside-baseline.json',
  proposal: './eastside-proposal.json',
};

const sources = {};         // name → SourceCitation (for citation rendering)
const reportCache = {};     // tab key → DisagreementReport

async function boot() {
  await init();

  // Populate sources map from library_info
  try {
    const info = JSON.parse(library_info());
    for (const op of info.opinions) {
      sources[op.name] = op.source;
    }
  } catch (e) {
    console.warn('library_info failed', e);
  }

  // Load both fixtures in parallel; render whichever's tab is active first.
  const [baseline, proposal] = await Promise.all([
    loadAndEvaluate('baseline'),
    loadAndEvaluate('proposal'),
  ]);
  reportCache.baseline = baseline;
  reportCache.proposal = proposal;

  renderReport('baseline', baseline);
  renderReport('proposal', proposal);

  // Wire tabs
  for (const tab of document.querySelectorAll('.tab')) {
    tab.addEventListener('click', () => {
      const target = tab.dataset.target;
      document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t === tab));
      document.querySelectorAll('.report').forEach(r => {
        r.classList.toggle('visible', r.id === `report-${target}`);
      });
    });
  }
}

async function loadAndEvaluate(key) {
  const res = await fetch(FIXTURES[key]);
  if (!res.ok) throw new Error(`Failed to load ${FIXTURES[key]}: ${res.status}`);
  const text = await res.text();
  const reportJson = evaluate_neighborhood(text);
  return JSON.parse(reportJson);
}

function renderReport(key, report) {
  const root = document.getElementById(`report-${key}`);
  if (!root) return;

  const html = [];

  // Two headline blocks — geometric and activist.
  html.push(headlineBlock('Geometric chorus', report.geometric_summary, 'geometric'));
  html.push(headlineBlock('Activist chorus', report.activist_summary, 'activist'));

  // Questions for humans
  if (report.questions_for_humans && report.questions_for_humans.length) {
    html.push('<section class="questions">');
    html.push('<h2>What to argue about</h2>');
    for (const q of report.questions_for_humans) {
      html.push(`
        <div class="question">
          <p class="q">${escapeHtml(q.question)}</p>
          <p class="why">${escapeHtml(q.why_it_matters)}</p>
        </div>
      `);
    }
    html.push('</section>');
  }

  // Individual opinions
  html.push('<section class="opinions">');
  html.push('<h2>The chorus — opinion by opinion</h2>');
  for (const ev of report.opinions) {
    html.push(renderOpinion(ev));
  }
  html.push('</section>');

  // Abstentions
  if (report.abstentions && report.abstentions.length) {
    html.push('<section class="abstentions">');
    html.push('<h2>What the algorithms refused to speak to</h2>');
    for (const a of report.abstentions) {
      html.push(`
        <div class="abstention">
          <span class="name">${escapeHtml(a.opinion_name)}</span>
          ${escapeHtml(a.reason)}
        </div>
      `);
    }
    html.push('</section>');
  }

  root.innerHTML = html.join('');
}

function headlineBlock(label, summary, cls) {
  return `
    <div class="headline-block ${cls}">
      <h2>${escapeHtml(label)}</h2>
      <p class="headline">${escapeHtml(summary.headline)}</p>
    </div>
  `;
}

function renderOpinion(ev) {
  const op = ev.opinion;
  const out = ev.output;
  const isValue = out.kind === 'value';

  const familyTag = isValue ? op.family : 'no_view';
  const familyLabel = isValue ? op.family : 'no view';

  const value = isValue
    ? `<div class="value">${out.value.toFixed(2)}</div>`
    : `<div class="value no_view">no view</div>`;

  const summary = isValue
    ? `<p class="summary">${escapeHtml(out.method_summary)}</p>`
    : `<p class="summary no_view">${escapeHtml(out.reason)}</p>`;

  let detailsBody = '';
  // Source citation
  const src = ev.opinion.source;
  detailsBody += `<p class="source"><strong>Voice:</strong> ${escapeHtml(src.display)}`;
  if (src.url) detailsBody += ` · <a href="${escapeAttr(src.url)}" target="_blank" rel="noopener">source</a>`;
  detailsBody += '</p>';

  // Caveats
  if (isValue && out.caveats && out.caveats.length) {
    detailsBody += '<p class="source"><strong>What this opinion does not see:</strong></p>';
    detailsBody += '<ul class="caveats">';
    for (const c of out.caveats) {
      detailsBody += `<li>${escapeHtml(c)}</li>`;
    }
    detailsBody += '</ul>';
  }

  return `
    <div class="opinion">
      <div class="name">${escapeHtml(prettyName(op.name))}</div>
      <span class="family-tag ${familyTag}">${escapeHtml(familyLabel)}</span>
      ${summary}
      ${value}
      <details>
        <summary>Source &amp; caveats</summary>
        ${detailsBody}
      </details>
    </div>
  `;
}

function prettyName(slug) {
  return slug.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

function escapeHtml(s) {
  if (s == null) return '';
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
function escapeAttr(s) {
  return escapeHtml(s).replace(/'/g, '&#39;');
}

boot().catch(err => {
  console.error(err);
  for (const root of document.querySelectorAll('.report')) {
    root.innerHTML = `<div class="loading">Failed to load: ${escapeHtml(err.message || String(err))}</div>`;
  }
});
