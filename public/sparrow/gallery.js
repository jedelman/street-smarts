// The gallery: a build's showcase shots (its showcase.json: tools/ghent/showcase.mjs chooses and photographs them).
// Tap a shot and you're there; the panel turns to why it was chosen. An archived build carries its own showcase
// beside its model, so walking a "before" shows the before's shots.

let state = null; // { sc, base, i } across openings

const esc = (s) => String(s == null ? "" : s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

export async function render(el, ctx) {
  if (!ctx.showcase) {
    el.innerHTML = "<p>This build has no showcase.</p>";
    return;
  }
  if (!state) {
    el.textContent = "Working it out…";
    const r = await fetch(ctx.showcase).catch(() => null);
    const sc = r && r.ok ? await r.json() : null;
    if (!sc || !Array.isArray(sc.shots) || !sc.shots.length) {
      el.innerHTML = "<p>This build has no showcase.</p>";
      return;
    }
    state = { sc, base: new URL(ctx.showcase, location.href), i: null };
  }
  const { sc, base } = state;
  const src = (s) => new URL(s.image, base).pathname;

  const grid = () => {
    state.i = null;
    el.innerHTML = `<p class="lede">${esc(sc.intro)}</p><ul class="gallery">${sc.shots.map((s, i) => `<li><button type="button" data-i="${i}">`
      + `<img src="${src(s)}" alt="" loading="lazy"><b>${esc(s.title)}</b><span>${esc(s.caption)}</span></button></li>`).join("")}</ul>`
      + `<p class="fine">${esc(sc.method)}</p>`;
    for (const b of el.querySelectorAll("button[data-i]")) b.addEventListener("click", () => go(+b.dataset.i));
  };
  const detail = (i) => {
    state.i = i;
    const s = sc.shots[i], n = sc.shots.length;
    el.innerHTML = `<p class="gallery-nav"><button type="button" class="linklike" data-back>&larr; All shots</button>`
      + `<span>${i + 1} of ${n}</span>`
      + `<button type="button" class="linklike" data-step="-1" aria-label="Previous shot">&lsaquo;</button>`
      + `<button type="button" class="linklike" data-step="1" aria-label="Next shot">&rsaquo;</button></p>`
      + `<figure class="photo"><img src="${src(s)}" alt="${esc(s.title)}, as the walk shows it."></figure>`
      + `<h3>${esc(s.title)}</h3>${(s.why || []).map((w) => `<p>${esc(w)}</p>`).join("")}`
      + (s.look_for ? `<p><b>Look for:</b> ${esc(s.look_for)}</p>` : "")
      + `<p><button type="button" class="linklike" data-again>Take me back there</button></p>`;
    el.querySelector("[data-back]").addEventListener("click", grid);
    for (const b of el.querySelectorAll("[data-step]")) b.addEventListener("click", () => go((i + +b.dataset.step + n) % n));
    el.querySelector("[data-again]").addEventListener("click", () => ctx.warp(s));
    el.scrollTop = 0;
    if (el.parentElement) el.parentElement.scrollTop = 0;
  };
  const go = (i) => {
    detail(i);
    ctx.warp(sc.shots[i]);
  };
  if (state.i != null) detail(state.i);
  else grid();
}
