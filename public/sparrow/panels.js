// The walk's flyout panels: one open at a time, from the right on a wide
// screen, from the bottom on a phone. The walk is the front page; the panels
// carry what the long read (read.html) says at length: the money, the
// iterations, the patterns you've met, the site map, how to play.
import { A, whole } from "./economics.js";

const $ = (id) => document.getElementById(id);
const money = (v) => (v >= 1e9 ? `$${(v / 1e9).toFixed(1)}B` : v >= 1e6 ? `$${Math.round(v / 1e6)}M` : `$${Math.round(v / 1e3)}k`);
const num = (v) => Math.round(v).toLocaleString("en-US");
const round = (v, to) => Math.round(v / to) * to;

/**
 * Wires the flyout (#flyout, its sections [data-panel], buttons [data-open]).
 * hooks: { open(name), close(name) }. Returns { open, close, toggle, get current }.
 */
export function setupPanels(hooks = {}) {
  const fly = $("flyout"), title = $("flyout-title");
  let current = null;
  const buttons = () => document.querySelectorAll("[data-open]");
  function open(name) {
    const sec = fly.querySelector(`[data-panel="${name}"]`);
    if (!sec) return;
    if (current && current !== name) close(true);
    for (const s of fly.querySelectorAll("[data-panel]")) s.hidden = s !== sec;
    title.textContent = sec.dataset.title || name;
    fly.hidden = false;
    fly.querySelector(".flyout-body").scrollTop = 0;
    current = name;
    for (const b of buttons()) b.setAttribute("aria-pressed", String(b.dataset.open === name));
    if (hooks.open) hooks.open(name, sec);
  }
  function close(switching = false) {
    if (!current) return;
    const was = current;
    current = null;
    if (!switching) fly.hidden = true;
    for (const b of buttons()) b.setAttribute("aria-pressed", "false");
    if (hooks.close) hooks.close(was);
  }
  const toggle = (name) => (current === name ? close() : open(name));
  for (const b of buttons()) b.addEventListener("click", () => toggle(b.dataset.open));
  $("flyout-close").addEventListener("click", () => close());
  return { open, close, toggle, get current() { return current; } };
}

/** The Money panel: the long read's headline, from the plan's measurements `n`. */
export function renderMoney(el, n) {
  if (!n) {
    el.innerHTML = `<p>The money is worked out for the whole site. <a href="/">Walk the whole site</a>, or read it in full on <a href="/read.html#adds-up">the long read</a>.</p>`;
    return;
  }
  const W = whole(n);
  const kpi = (big, small) => `<div class="kpi"><b>${big}</b><span>${small}</span></div>`;
  el.innerHTML = `
    <p class="lede">Public finance before it's retail: what it would cost, and who would pay.</p>
    <div class="kpis">
      ${kpi(money(W.cost.total), `to build: homes ${money(W.cost.homes)}, work ${money(W.cost.work)}, shops ${money(W.cost.shops)}, land ${money(A.landPaid)} at what the public paid. Before streets and utilities.`)}
      ${kpi("4 or 5", `phases of ${money(W.cost.total / 5)} to ${money(W.cost.total / 4)} each (our guess).`)}
      ${kpi(money(W.noi.total), `a year after running costs at today's Norfolk rents: ${(W.noi.total / W.cost.total * 100).toFixed(1)}% of what it costs. Buyers ask 5.6&ndash;7.7%.`)}
      ${kpi(`${(W.payRate * 100).toFixed(1)}%`, "the interest rate that pays it all back in 30 years. Norfolk borrows at up to 4.1%.")}
      ${kpi(`${Math.round(W.freeNeed * 100)}%`, "of the money as grants and tax credits for it to pay its way, the city lending a third of the rest. One St. Paul's building got 41%.")}
      ${kpi(money(W.tax), "a year in property tax. The land stays the public's.")}
    </div>
    <p class="small">In it: about ${num(round(W.homes, 100))} homes, ${num(round(W.desks, 100))} desks and ${num(n.shop_fronts)} shop fronts. Homes and desks would spend ${money(W.spend + W.deskSpend)} a year nearby; switch on <b>Flows</b> to see where.</p>
    <p><a class="more" href="/read.html#adds-up">The whole analysis, sourced: how Norfolk paid for St. Paul's, a stack you can move, rent three ways &rarr;</a></p>`;
}

/** The Iterations panel: every archived version, newest first, each one walkable. */
export function renderIterations(el, its, currentId) {
  const short = (d) => new Date(d + "T12:00:00").toLocaleDateString("en-US", { month: "short", day: "numeric" });
  const li = (href, img, head, body, here) => `<li${here ? ' class="here"' : ""}>${img ? `<img src="${img}" alt="" loading="lazy">` : ""}<div><b>${head}</b>${body}${here ? ' <em>You&rsquo;re here.</em>' : ` <a href="${href}">Walk it</a>`}</div></li>`;
  const esc = (s) => String(s || "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  el.innerHTML = `<ol class="its">${li("/", null, "Now", " The plan as it is today.", !currentId)}${its.slice().reverse().map((it) => {
    const id = encodeURIComponent(it.id);
    const href = `/?glb=/archive/${id}/clean_baseline.glb&chorus=/archive/${id}/chorus.json&iteration=${id}`;
    const img = (it.files || []).includes("isometric.png") ? `/archive/${id}/isometric.png` : null;
    return li(href, img, `${short(it.date)} · ${esc(it.title)}`, ` ${esc(it.summary)}${it.pr ? ` (PR #${it.pr})` : ""}`, it.id === currentId);
  }).join("")}</ol>`;
}
