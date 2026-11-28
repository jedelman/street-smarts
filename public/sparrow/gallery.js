// The gallery: a build's showcase shots (its showcase.json: tools/ghent/showcase.mjs chooses and photographs them).
// Tap a shot and you're there; the panel turns to why it was chosen. An archived build carries its own showcase
// beside its model, so walking a "before" shows the before's shots.

let state = null; // { sc, base, i, seen } across openings
let loading = null;

/** The build's showcase.json, loaded once. */
async function load(url) {
  if (state) return state;
  if (!loading) {
    loading = fetch(url).then((r) => (r && r.ok ? r.json() : null)).catch(() => null).then((sc) => {
      if (sc && Array.isArray(sc.shots) && sc.shots.length) state = { sc, base: new URL(url, location.href), i: null, seen: new Set() };
      return state;
    });
  }
  return loading;
}

/**
 * In context (main.js surface): walking up to where a shot was taken (within 14 m of where it stands or what it looks
 * at), that shot and why it was chosen. Not a shot you've already opened here.
 */
export async function peek(ctx) {
  if (ctx.reason !== "near" || !ctx.showcase || !ctx.here) return null;
  const st = await load(ctx.showcase);
  if (!st) return null;
  let best = -1, bd = 14;
  st.sc.shots.forEach((s, i) => {
    const d = Math.min(Math.hypot(s.stand[0] - ctx.here.x, s.stand[1] - ctx.here.z), Math.hypot(s.look[0] - ctx.here.x, s.look[2] - ctx.here.z));
    if (d < bd && !st.seen.has(i)) { bd = d; best = i; }
  });
  if (best < 0) return null;
  const s = st.sc.shots[best];
  return { key: `gallery:${s.id}`, text: `${s.title}. ${s.caption}`, open: () => { st.i = best; } };
}

const esc = (s) => String(s == null ? "" : s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

export async function render(el, ctx) {
  if (!ctx.showcase) {
    el.innerHTML = "<p>This build has no showcase.</p>";
    return;
  }
  if (!state) {
    el.textContent = "Working it out…";
    if (!(await load(ctx.showcase))) {
      el.innerHTML = "<p>This build has no showcase.</p>";
      return;
    }
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
    state.seen.add(i);
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
