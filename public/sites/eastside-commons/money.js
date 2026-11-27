// Eastside Commons' Money panel: the long read's headline, worked out from the
// plan's own measurements (numbers.json, from tools/vibe-render/plan_numbers.py)
// by this site's greenfield model (economics.js). The walk calls render() the
// first time the panel opens; site.json names this module.
import { A, whole } from "./economics.js";

const money = (v) => (v >= 1e9 ? `$${(v / 1e9).toFixed(1)}B` : v >= 1e6 ? `$${Math.round(v / 1e6)}M` : `$${Math.round(v / 1e3)}k`);
const num = (v) => Math.round(v).toLocaleString("en-US");
const round = (v, to) => Math.round(v / to) * to;

/** The Money panel: the long read's headline, from the plan's measurements `n`. */
export function render(el, { numbers: n }) {
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
