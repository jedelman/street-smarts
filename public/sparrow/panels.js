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
