// The walk's flyout panels: one open at a time, from the right on a wide
// screen, from the bottom on a phone. Every site has the map, the patterns
// or places you've met, the tours and how to play; a site's own panels (the
// money, say) are rendered by modules its site.json names.

const $ = (id) => document.getElementById(id);

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

/** The Tours panel, for a site with more than one: each tour's title and note, and a button to start it. */
export function renderTours(el, tours, start) {
  el.innerHTML = "";
  const ol = document.createElement("ol");
  ol.className = "tours";
  for (const def of tours) {
    const li = document.createElement("li");
    const b = document.createElement("button");
    b.type = "button";
    b.className = "linklike";
    b.textContent = def.title;
    b.addEventListener("click", () => start(def));
    li.append(b);
    if (def.note) { const p = document.createElement("p"); p.className = "small"; p.textContent = def.note; li.append(p); }
    ol.append(li);
  }
  el.append(ol);
}

/** The Iterations panel: every archived version, newest first, each one walkable. */
export function renderIterations(el, its, currentId, page = "/") {
  const short = (d) => new Date(d + "T12:00:00").toLocaleDateString("en-US", { month: "short", day: "numeric" });
  const li = (href, img, head, body, here) => `<li${here ? ' class="here"' : ""}>${img ? `<img src="${img}" alt="" loading="lazy">` : ""}<div><b>${head}</b>${body}${here ? ' <em>You&rsquo;re here.</em>' : ` <a href="${href}">Walk it</a>`}</div></li>`;
  const esc = (s) => String(s || "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  el.innerHTML = `<ol class="its">${li(page, null, "Now", " The plan as it is today.", !currentId)}${its.slice().reverse().map((it) => {
    const id = encodeURIComponent(it.id);
    // A site bundle's build (tools/sparrow/archive_site.py) names its folder and files; Eastside's milestones keep
    // the older layout (tools/vibe-render/archive_milestone.py).
    const href = iterationHref(it, page);
    const img = it.folder ? (it.thumb ? it.folder + it.thumb : null)
      : (it.files || []).includes("isometric.png") ? `/archive/${id}/isometric.png` : null;
    return li(href, img, `${short(it.date)} · ${esc(it.title)}`, ` ${esc(it.summary)}${it.pr ? ` (PR #${it.pr})` : ""}`, it.id === currentId);
  }).join("")}</ol>`;
}

/** The address that walks an archived build. */
export function iterationHref(it, page = "/") {
  const id = encodeURIComponent(it.id);
  if (!it.folder) return `/?glb=/archive/${id}/clean_baseline.glb&chorus=/archive/${id}/chorus.json&iteration=${id}`;
  const q = [`glb=${it.folder}${it.model || "model.glb"}`];
  if (it.chorus) q.push(`chorus=${it.folder}${it.chorus}`);
  q.push(`iteration=${id}`);
  return `${it.page || page}?${q.join("&")}`;
}

/**
 * On a phone, the HUD's panel buttons fold into one: "Panels" opens a menu of them, grouped (each button's
 * data-group; the groups' names and order are #hud-menu's data-groups, as JSON pairs). A button that's hidden (a
 * panel the site or the moment doesn't offer) is left out; the menu is rebuilt each time it opens, so one that
 * appears later (the Lot, say) shows up. The buttons themselves do the work: the menu clicks them.
 */
export function setupHudMenu() {
  const btn = $("menu-btn"), menu = $("hud-menu");
  if (!btn || !menu) return;
  let groups = [];
  try { groups = JSON.parse(menu.dataset.groups || "[]"); } catch { /* none */ }
  const close = () => { menu.hidden = true; btn.setAttribute("aria-expanded", "false"); };
  const build = () => {
    menu.innerHTML = "";
    for (const [key, label] of groups) {
      const items = [...document.querySelectorAll(`.hud.top-right [data-group="${key}"]`)].filter((b) => !b.hidden);
      if (!items.length) continue;
      const h = document.createElement("p");
      h.className = "group";
      h.textContent = label;
      const row = document.createElement("div");
      row.className = "row";
      for (const b of items) {
        const c = document.createElement("button");
        c.type = "button";
        c.textContent = b.textContent.trim();
        c.setAttribute("aria-pressed", b.getAttribute("aria-pressed") || "false");
        if (b.title) c.title = b.title;
        c.addEventListener("click", () => { close(); b.click(); });
        row.append(c);
      }
      menu.append(h, row);
    }
  };
  btn.addEventListener("click", () => {
    if (!menu.hidden) return close();
    build();
    menu.hidden = false;
    btn.setAttribute("aria-expanded", "true");
  });
  addEventListener("keydown", (e) => { if (e.key === "Escape" && !menu.hidden) close(); });
  addEventListener("pointerdown", (e) => { if (!menu.hidden && !menu.contains(e.target) && !btn.contains(e.target)) close(); });
}
