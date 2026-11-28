// "Day to day": the proposal in a neighbour's terms. From the lot you last touched (or where you're standing): what
// rent is here, how far a walk it is to coffee, groceries or a haircut today and with the proposal, how many are
// within an n-minute walk, and how often something new opens nearby. The arithmetic is walkgraph.js; the data is
// everyday.json, with its sources.
import { prepare, snap, summary, tractAt, minutesOf, everyOf, everyRange } from "./walkgraph.js";

let data = null;
let minutes = 10;
let from = "lot"; // or "here"

const BIZ = { coffee: "a coffee shop", bistro: "a bistro", bakery: "a bakery", books: "a bookshop", greengrocer: "a greengrocer's",
  florist: "a florist's", hardware: "a hardware store", bikes: "a bike shop", records: "a record shop", hairdresser: "a hairdresser's",
  barber: "a barbershop", cleaners: "a dry cleaner's", tailor: "a tailor's", accountant: "an accountant's office",
  architect: "an architects' studio", design: "a design studio", dentist: "a dentist's office", gallery: "an art gallery",
  yoga: "a yoga studio", wine: "a wine shop" };
const money = (n) => `$${Math.round(n).toLocaleString("en-US")}`;
const pct = (f) => `${Math.round(f * 100)}%`;
const list = (xs) => (xs.length < 2 ? xs.join("") : `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`);

export async function render(el, ctx) {
  if (!data) {
    el.textContent = "Working it out…";
    data = prepare(await (await fetch(new URL("./everyday.json", import.meta.url))).json());
  }
  const factsOf = new Map(ctx.elements.filter((e) => e.source && e.source.startsWith("parcel:")).map((e) => [e.source.slice(7), e]));
  const lotEl = ctx.place && ctx.place.source && ctx.place.source.startsWith("parcel:") ? ctx.place : null;
  const lot = lotEl && data.lots[lotEl.source.slice(7)];
  const useLot = from === "lot" && lot;
  let origin, tract, title;
  if (useLot) {
    origin = lot.slice(0, 3);
    tract = lot[3];
    title = (lotEl.facts && lotEl.facts.name) || lotEl.name || "the lot you touched";
  } else {
    const x = ctx.here ? ctx.here.x : 0, y = ctx.here ? -ctx.here.z : 0;
    origin = snap(data, x, y);
    tract = tractAt(data, x, y);
    title = "where you're standing";
  }
  const S = summary(data, origin, minutes);
  el.replaceChildren();
  const p = (html, cls) => { const q = document.createElement("p"); if (cls) q.className = cls; q.innerHTML = html; el.append(q); return q; };
  const h = (text) => { const q = document.createElement("h3"); q.textContent = text; el.append(q); };

  // Where from, and how far a walk.
  const head = p(`From <b>${title}</b>.`);
  if (lot && ctx.here) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "linklike";
    b.textContent = useLot ? "From where I'm standing instead" : "From the lot I touched";
    b.addEventListener("click", () => { from = useLot ? "here" : "lot"; render(el, ctx); });
    head.append(" ", b);
  }
  const pick = document.createElement("p");
  pick.className = "choose";
  pick.append("A walk of ");
  for (const n of [5, 10, 15]) {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = `${n} minutes`;
    b.setAttribute("aria-pressed", String(n === minutes));
    b.addEventListener("click", () => { minutes = n; render(el, ctx); });
    pick.append(b, " ");
  }
  el.append(pick);

  // Rent.
  h("Rent");
  const t = data.tracts[tract];
  if (t) {
    p(`The median rent in ${t.name} is <b>${money(t.median_rent)} a month</b> (give or take ${money(t.median_rent_moe)}). `
      + `${pct(t.renting)} of its ${t.homes.toLocaleString("en-US")} homes are rented, and ${pct(t.rent_burdened)} of its renters pay `
      + `30% or more of their income in rent.`);
  } else {
    p("This spot is outside the census tracts the League's area covers.");
  }
  p("What the proposal would do to rents isn't in this data. Policy Brief No.&nbsp;1, on its effect nearby: "
    + "“Whether they also raise nearby home values is less clear; the research is mixed.”", "small");

  // A walk to...
  const [lo, hi] = S.share;
  const range = (a, b) => (a === b ? `${a}` : `${a}–${b}`);
  const paced = (base, added) => range(base + Math.round(added * lo), base + Math.round(added * hi));
  const table = (caption, heads, rows) => {
    const t = document.createElement("table");
    t.className = "everyday";
    t.innerHTML = `<caption>${caption}</caption><thead><tr><th scope="col"></th>${heads.map((x) => `<th scope="col">${x}</th>`).join("")}</tr></thead>`;
    const tb = document.createElement("tbody");
    for (const [label, cells, changed] of rows) {
      const tr = document.createElement("tr");
      if (changed) tr.className = "changed";
      tr.innerHTML = `<th scope="row">${label}</th>${cells.map((c) => `<td>${c}</td>`).join("")}`;
      tb.append(tr);
    }
    t.append(tb);
    el.append(t);
  };
  h("The walk to…");
  table("The nearest, on foot", ["Today", "If every ground floor opened"],
    S.kinds.map((k) => [k.label, [minutesOf(k.today.nearest, S.mps), minutesOf(k.proposed.nearest, S.mps)], k.proposed.nearest < k.today.nearest - 30]));
  table(`How many within a ${minutes}-minute walk`, ["Today", "At Brief 2's pace", "If every one opened"],
    S.kinds.map((k) => [k.label, [k.today.within, paced(k.today.within, k.added), k.today.within + k.added], k.added > 0]));
  p(`The map opens every ground floor the proposal would allow. Policy Brief No.&nbsp;2 expects far fewer to open: its model `
    + `assumes “${data.pace.says}”, of the ${data.pace.of} the map opens in HC-G2. “At Brief 2's pace” applies that share to `
    + `every ground floor here (${pct(lo)} to ${pct(hi)}); that's our assumption, not the brief's.`, "small");

  // Something new.
  h("Something new");
  const openedN = S.opened.food + S.opened.shop + S.opened.service;
  const since = new Date(data.opened.from).getFullYear();
  p(openedN
    ? `Since January ${since}, the City has licensed <b>${openedN}</b> new eateries, shops and services within a ${minutes}-minute walk of here: `
      + `<b>${everyOf(S.openedPerYear)}</b>.`
    : `The City hasn't licensed a new eatery, shop or service within a ${minutes}-minute walk of here since January ${since}.`);
  const nearN = S.near.food + S.near.shop + S.near.service;
  if (nearN) {
    const nLo = Math.round(nearN * lo), nHi = Math.round(nearN * hi);
    p(`The map opens <b>${nearN}</b> ground floor${nearN === 1 ? "" : "s"} within that walk; at Brief 2's pace, about ${range(nLo, nHi)} of them `
      + `would be open. Each year about ${pct(data.bds.food.entry)} of the Virginia Beach–Norfolk metro's eateries, `
      + `${pct(data.bds.service.entry)} of its services and ${pct(data.bds.shop.entry)} of its shops are new. At those rates, with what `
      + `opens now: something new <b>${everyRange(S.openedPerYear + S.newPerYear * hi, S.openedPerYear + S.newPerYear * lo)}</b> `
      + `at Brief 2's pace, and ${everyOf(S.openedPerYear + S.newPerYear)} if every one opened.`);
  }
  const mine = S.onBlock.proposed;
  if (mine.length) {
    const what = mine.map((q) => BIZ[(factsOf.get(q.gpin)?.facts || {}).business] || "a ground floor");
    p(`On this block, between the corners, the map opens ${list(what)}. At Brief 2's pace, each has about a ${pct(lo)} to ${pct(hi)} `
      + `chance of opening. If all ${mine.length === 1 ? "of it opened" : `${mine.length} opened`}, one would be new ${everyOf(S.blockPerYear)}.`);
  } else {
    p("The map opens no ground floor on this block, between the corners.");
  }
  p(`Walks run along the streets at 3 miles an hour (ours); crossings and alleys aren't modelled. Today's places: `
    + `${data.sources.places}; they may miss a few and keep some that have closed. Licences: ${data.sources.opened} (it doesn't say `
    + `which have closed, and a few may be run from home). Rent: ${data.sources.rent}. Rates: ${data.sources.bds}.`, "fine");
}
