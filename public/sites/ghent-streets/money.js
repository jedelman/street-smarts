// Ghent Streets' Money panel, at three scales side by side: what the city pays and gets, what a household here pays and
// gets, and what a ground floor would rent for, today and after the change. Adaptive reuse, so nothing is built.
//   - The city: Policy Brief No. 2's figures, quoted, not recomputed (it prices 15 to 40 businesses that might open;
//     the walk shows every ground floor that could).
//   - A household: the census tract's rent (ACS 2020-2024), and the places within a walk, from Day to day's data
//     (everyday.js, walkgraph.js), from the lot last touched or where you stand.
//   - How rent might change: a range from studies of home prices near new restaurants, cafés and shops, quoted and
//     checked against their sources; no good study measures rents directly, and the panel says so.
// Live: it renders afresh each time it opens.
import { loadEveryday, locate } from "./everyday.js";
import { summary } from "./walkgraph.js";
import { indexed, range, monthly } from "./shoprent.js";

const BRIEF2 = "https://jason-edelman.org/ghent-streets/policy/brief-02.html";
const BRIEF1 = "https://jason-edelman.org/ghent-streets/policy/brief-01.html";
// The upper end of the rent range: Liu & Zhao (2026), house prices about 3.4-3.7% higher four or five years after a
// local wave of new food and drink places (London; a preprint). Applied to the tract's median rent, as illustration.
const RENT_HIGH = 0.037;
const STUDIES = {
  liu: { cite: "Liu & Zhao 2026, a preprint not yet peer-reviewed", url: "https://arxiv.org/pdf/2603.03260v2" },
  boyle: { cite: "Boyle, Barrilleaux & Scheller 2014", url: "https://ideas.repec.org/a/bla/socsci/v95y2014i3p852-867.html" },
  sohn: { cite: "Sohn, Moudon & Lee 2012", url: "https://softlive.floridadep.gov/sites/default/files/Economic%20Value%20of%20walkable%20neighborhoods%2C%20Sohn%2C%20Moudon%20and%20Lee%2C%202012_0.pdf" },
  pivo: { cite: "Pivo & Fisher 2011", url: "https://experts.arizona.edu/en/publications/the-walkability-premium-in-commercial-real-estate-investments/" },
  couture: { cite: "Couture, Gaubert, Handbury & Hurst 2019", url: "https://www.nber.org/papers/w26142" },
  li: { cite: "Li 2019, a working paper", url: "https://blocksandlots.com/wp-content/uploads/2020/02/Do-New-Housing-Units-in-Your-Backyard-Raise-Your-Rents-Xiaodi-Li.pdf" },
};
const money = (v) => `$${Math.round(v).toLocaleString("en-US")}`;
const esc = (x) => String(x == null ? "" : x).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const pct = (f) => `${Math.round(f * 100)}%`;
const src = (k) => `(<a href="${STUDIES[k].url}" target="_blank" rel="noopener">${STUDIES[k].cite}</a>)`;
let shopRents = null;
const loadShopRents = () => shopRents || (shopRents = fetch(new URL("./shoprents.json", import.meta.url)).then((r) => (r.ok ? r.json() : null)).catch(() => null));

/**
 * In context (main.js surface): touching a lot, what it would rent for as a shop if the proposal converts it (by
 * comparison, as the panel's ground-floor card works it out), else the rent a household pays here today.
 */
export async function peek(ctx) {
  const el = ctx.reason === "touch" && ctx.touched;
  if (!el || !el.source || !el.source.startsWith("parcel:") || !el.facts) return null;
  const gpin = el.source.slice(7);
  const d = await loadEveryday();
  if (el.facts.status === "converted") {
    const sr = await loadShopRents();
    const zip = sr && sr.lot_zips[gpin], sqft = d.units && d.units[gpin];
    if (!zip || !sqft || !sr.ghent_zips.some((g) => g.zip === zip)) return null;
    const r = range(indexed(sr), zip);
    return { key: `money:${gpin}`, text: `As a shop, its ${sqft.toLocaleString("en-US")} sq ft would rent for about ${money(monthly(r.median, sqft))} a month, going by districts like it in other cities.` };
  }
  const lot = d.lots[gpin], t = lot && d.tracts[lot[3]];
  if (!t) return null;
  return { key: `money:${gpin}`, text: `Rent here is ${money(t.median_rent)} a month, the tract's median. What the change might do to it, and what the city gets.` };
}

export async function render(el, ctx) {
  el.textContent = "Working it out…";
  const [d, n, sr] = await Promise.all([
    loadEveryday(),
    fetch(new URL("./numbers.json", import.meta.url)).then((r) => (r.ok ? r.json() : null)).catch(() => null),
    loadShopRents(),
  ]);
  const at = locate(d, ctx), t = d.tracts[at.tract];
  const S = summary(d, at.origin, 10);
  const [lo, hi] = S.share;
  const today = S.kinds.reduce((s, k) => s + k.today.within, 0), added = S.kinds.reduce((s, k) => s + k.added, 0);
  const near = (id, side) => S.kinds.find((k) => k.id === id)[side].nearest / S.mps / 60;
  const mins = (m) => (!isFinite(m) || m > 15 ? "over 15 minutes" : m < 1 ? "under a minute" : Math.round(m) === 1 ? "a minute" : `${Math.round(m)} minutes`);
  const card = (title, sub, rows, [la, lb] = ["Pays", "Gets"]) => `<section class="scale"><h3>${title}</h3>${sub ? `<p class="small">${sub}</p>` : ""}`
    + rows.map(([when, pays, gets]) => `<h4>${when}</h4><dl><dt>${la}</dt><dd>${pays}</dd><dt>${lb}</dt><dd>${gets}</dd></dl>`).join("") + "</section>";

  const city = card("The city", "Norfolk, for these ground floors", [
    ["Today", "Nothing new.",
      "The property tax these homes already pay. No meals, sales or business-licence tax: today no permit can allow a café or shop on these ground floors."],
    ["After the change", `To adopt it, in Policy Brief No.&nbsp;2's words: &ldquo;staff time, legal review and printing.&rdquo;`,
      `<b>$194k&ndash;$488k a year</b> in taxes from 15 to 40 new ground-floor businesses (Brief 2's scenarios, gross, before spending that would have happened elsewhere in Norfolk anyway); $227k&ndash;$602k with its property-value lines. A café at $350,000 in sales pays $22,750 a year in meals tax (6.5%).`],
  ]);
  const household = t
    ? card("A household here", `From ${at.title}, in ${t.name}`, [
      ["Today", `<b>${money(t.median_rent)} a month</b>, the tract's median rent (give or take ${money(t.median_rent_moe)}). ${pct(t.rent_burdened)} of its renters pay 30% or more of their income in rent.`,
        `<b>${today}</b> places within a 10-minute walk (of Day to day's 17 kinds). Coffee ${mins(near("coffee", "today"))} away; groceries ${mins(near("groceries", "today"))}.`],
      ["After the change", `From <b>no change to about ${money(t.median_rent * RENT_HIGH)} a month more</b> after four or five years, if rents moved as nearby home prices did in the studies below; perhaps less right next door to a new business. Illustrative: no good study measures rents directly.`,
        `<b>${today + Math.round(added * lo)}&ndash;${today + Math.round(added * hi)}</b> places within 10 minutes at Brief 2's pace; ${today + added} if every ground floor opened, with coffee ${mins(near("coffee", "proposed"))} away and groceries ${mins(near("groceries", "proposed"))}.`],
    ])
    : card("A household here", "This spot is outside the census tracts the League's area covers.", []);

  // The ground floor: what it might rent for as a shop, by comparison, indexed to home values (shoprent.js).
  let floor = "", floorHow = "";
  if (sr) {
    const ix = indexed(sr);
    const gpin = at.useLot && at.el.source.slice(7);
    const zip = (gpin && sr.lot_zips[gpin]) || null;
    const z = sr.ghent_zips.find((g) => g.zip === zip) || null;
    const sqft = (gpin && d.units && d.units[gpin]) || 1500;
    const biz = gpin && at.el.facts && at.el.facts.business;
    const BIZ = { coffee: "a coffee shop", bistro: "a bistro", bakery: "a bakery", books: "a bookshop", greengrocer: "a greengrocer's",
      florist: "a florist's", hardware: "a hardware store", bikes: "a bike shop", records: "a record shop", hairdresser: "a hairdresser's",
      barber: "a barbershop", cleaners: "a dry cleaner's", tailor: "a tailor's", accountant: "an accountant's office", architect: "an architects' studio",
      design: "a design studio", dentist: "a dentist's office", gallery: "an art gallery", yoga: "a yoga studio" };
    const rangeLine = (g) => {
      const r = range(ix, g.zip);
      return `about <b>${money(monthly(r.median, sqft))} a month</b> (${money(r.median)} a sq ft a year), between ${money(monthly(r.low, sqft))} and ${money(monthly(r.high, sqft))} across the comparables`;
    };
    const after = z
      ? `Its ${sqft.toLocaleString("en-US")} sq ft would rent for ${rangeLine(z)}, indexed to its ZIP, ${z.zip}.`
      : `${sqft.toLocaleString("en-US")} sq ft would rent for ${sr.ghent_zips.map((g) => `${rangeLine(g)} in ${g.zip} (${g.label})`).join("; ")}. Touch a lot for its own.`;
    floor = card("The ground floor", gpin ? `Of ${at.title}` : "Anywhere in the League's area", [
      ["Today", "Nothing as a shop: the use isn't allowed.", "Part of a home, or nothing."],
      ["After the change", after, `${biz ? (BIZ[biz] || "A shop").replace(/^./, (c) => c.toUpperCase()) : "A shop"}, behind the doors and windows it has. Triple net: the tenant also pays its share of taxes, insurance and upkeep.`],
    ], ["Rents for", "Holds"]);
    const zrow = z || sr.ghent_zips[0];
    floorHow = `<h3>What a ground floor would rent for</h3>
      <p>Ghent's historic district has no shop market to look at: today the use is prohibited. So the estimate borrows from neighborhoods like it in other cities. Each one's asking rent is divided by its ZIP code's typical home value, and that ratio is applied to Ghent's (${esc(sr.home_values)}).</p>
      <table class="everyday rents"><caption>The comparables, and what each says for ZIP ${zrow.zip} (${money(zrow.value)})</caption>
      <thead><tr><th scope="col"></th><th scope="col">Asking, NNN</th><th scope="col">Typical home</th><th scope="col">Per $100k</th><th scope="col">Here</th></tr></thead><tbody>
      ${ix.rows.map((r) => `<tr><th scope="row">${esc(r.city.replace(/, [A-Z]{2}$/, ""))}<span class="small">${esc(r.district)}</span></th><td>${r.rent_low === r.rent_high ? money(r.rent_low) : `${money(r.rent_low)}&ndash;${money(r.rent_high)}`}</td><td>${money(r.home_value / 1000)}k</td><td>$${r.ratio.toFixed(2)}</td><td>${money(r.ghent.find((g) => g.zip === zrow.zip).rent)}</td></tr>`).join("")}
      </tbody></table>
      <ul class="evidence">${ix.rows.map((r) => `<li><b>${esc(r.city)}, ${esc(r.district)}:</b> ${r.listings.map(esc).join("; ")}. <a href="${r.url}" target="_blank" rel="noopener">${esc(r.source)}</a>, ${esc(r.date)}; checked ${esc(r.checked)}.${r.notes ? ` ${esc(r.notes)}` : ""}</li>`).join("")}</ul>
      <p>Against Ghent's own asking rents: ${sr.checks.map((c) => `${esc(c.district)}, ${money(c.rent_low)} a sq ft (${esc(c.lease_type)}, ${esc(c.date)}; <a href="${c.url}" target="_blank" rel="noopener">${esc(c.source)}</a>; checked ${esc(c.checked)})`).join("; ")}.</p>
      ${sr.left_out.map((x) => `<p class="small">Left out: ${esc(x.city)}. ${esc(x.why)}</p>`).join("")}
      <p class="small">${esc(sr.rule)}</p>`;
  }
  el.innerHTML = `
    <p class="lede">What we pay and what we get, today and after the change: for the city, for a household here, and for the ground floor.</p>
    <div class="scales">${city}${household}</div>
    ${floor ? `<div class="scales one">${floor}</div>` : ""}
    <h3>How rent might change</h3>
    <p>No good study measures what small businesses opening nearby do to rents. The evidence is on home prices, and it runs from nothing to a few percent:</p>
    <ul class="evidence">
      <li>In London, after a wave of new restaurants and cafés nearby, &ldquo;log house prices rise gradually from about 0.5% in the event year to roughly 3.4&ndash;3.7% by years four and five&rdquo; ${src("liu")}. That upper end, as a share of this tract's median rent, is the figure above.</li>
      <li>In Miami, comparing homes within the same neighborhood, the kind of change this would be, &ldquo;walkability's impact on housing value becomes statistically insignificant at the margin after controlling for heteroscedasticity and neighborhood fixed effects&rdquo; ${src("boyle")}.</li>
      <li>In King County, &ldquo;proximity to retail service was negatively associated with assessed property value for rental housing&rdquo;, while more retail across the neighborhood was associated with higher values ${src("sohn")}.</li>
      <li>Values aren't rents. In Pivo and Fisher's study of offices, shops and apartments, &ldquo;a 10-point increase in walkability increased values by 1-9%, depending on property type&rdquo; ${src("pivo")}: what owners' buildings are worth, not what renters pay.</li>
    </ul>
    <p>Two things weigh on renters either way. A rent rise falls on everyone, including people who never use the new places: renters may &ldquo;pay higher rents for amenities that they do not value as much&rdquo; ${src("couture")}. And a ground floor that's a home now and becomes a shop is one less home; more housing tends to mean lower rents nearby (&ldquo;for every 10% increase in the housing stock, rents decrease 1%&rdquo; ${src("li")}).</p>
    ${floorHow}
    <p>What it costs an owner to open: under the amendment, a café in an existing ground floor needs a zoning certificate, and Architectural Review Board review for any exterior work (<a href="${BRIEF1}" target="_blank" rel="noopener">Policy Brief No.&nbsp;1</a>). Today no permit can allow it.</p>
    ${n ? `<p class="small">In this model: ${n.converted_by_kind.food} cafés and restaurants, ${n.converted_by_kind.retail} shops and ${n.converted_by_kind.services} services, in the brief's stated mix, each at most 3,500 sq ft. The walk shows the ceiling; the brief prices the 15 to 40 that might open. Rent: ${d.sources.rent}.</p>` : ""}
    <p><a class="more" href="${BRIEF2}" target="_blank" rel="noopener">The city's side, sourced: Policy Brief No.&nbsp;2 &rarr;</a></p>`;
}
