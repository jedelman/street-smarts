// Ghent Streets' Corners panel: the corner lots the campaign identified in HC-G2 (the explorer's rule: a lot
// within 45 ft of two differently named streets that meet within 120 ft of it), by street and number. Each flies
// you there; the proposal opens the ground floor of every one with a building to adapt, and the shop that opens
// there is named as its sign reads in the walk (frontages.js; the names are ours).
const STATUS = {
  converted: "opens",
  existing: "already commercial",
  "nothing-to-adapt": "no building to adapt",
  "not-eligible": "not in the proposal",
};

export function render(el, { site, elements, goToElement, shops = [] }) {
  const shopName = new Map(shops.filter((sh) => sh.building && sh.name).map((sh) => [sh.building, sh.name]));
  const bySource = new Map();
  for (const e of elements) {
    if (e.group !== "buildings" || !e.source) continue;
    const cur = bySource.get(e.source);
    if (!cur || (e.area || 0) > (cur.area || 0)) bySource.set(e.source, e);
  }
  const lots = new Map(elements.filter((e) => e.group === "lots").map((e) => [e.source, e]));
  el.innerHTML = "";
  const p = document.createElement("p");
  p.className = "small";
  p.textContent = `${site.corners.length} corner lots in HC-G2. Choose one to fly there and read it. The shops' names are ours, as their signs read in the walk.`;
  el.append(p);
  const ol = document.createElement("ol");
  ol.className = "corners";
  let street = null;
  for (const c of site.corners) {
    const target = bySource.get(c.element) || lots.get(c.element);
    const facts = target && target.facts;
    const name = c.street || (c.label || "").replace(/^\d+[A-Z]?\s+/, "");
    if (name !== street) {
      street = name;
      const h = document.createElement("li");
      h.className = "street";
      h.textContent = name;
      ol.append(h);
    }
    const li = document.createElement("li");
    const b = document.createElement("button");
    b.type = "button";
    b.className = "linklike";
    b.textContent = c.label;
    b.disabled = !target;
    b.addEventListener("click", () => goToElement(target));
    li.append(b);
    const named = shopName.get(c.element);
    if (named) {
      const n = document.createElement("b");
      n.className = "shop";
      n.textContent = named;
      li.append(" ", n);
    }
    const st = c.note || (facts && STATUS[facts.status]);
    if (st) {
      const s = document.createElement("span");
      s.className = `status ${(facts && facts.status) || ""}`;
      s.textContent = st;
      li.append(" ", s);
    }
    ol.append(li);
  }
  el.append(ol);
}
