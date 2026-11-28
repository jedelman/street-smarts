// Panels offered in context (main.js surface, panels.js setupPeek): every panel a site lists in `surface` has a
// module that can peek, and the peeks that need no network say the right thing at the right time.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const SITE_DIR = new URL("../../public/sites/ghent-streets/", import.meta.url);
const site = JSON.parse(readFileSync(new URL("site.json", SITE_DIR), "utf8"));
const moduleUrl = (mod) => (mod.startsWith("/") ? new URL(`../../public${mod}`, import.meta.url) : new URL(mod, SITE_DIR));

test("every panel Ghent surfaces has a module that can peek", async () => {
  assert.ok(Array.isArray(site.surface) && site.surface.length, "a surface list");
  for (const name of site.surface) {
    assert.ok(site.renderers[name], `${name} has a renderer`);
    const m = await import(moduleUrl(site.renderers[name]));
    assert.equal(typeof m.peek, "function", `${name} exports peek`);
  }
});

test("the Lot offers itself only for a lot the assessor photographed, and only on a touch", async () => {
  const { peek } = await import(moduleUrl(site.renderers.place));
  const lot = { source: "parcel:1", facts: { name: "1 Example Street", photo: "https://example.org/1.jpg" } };
  assert.match(peek({ touched: lot, reason: "touch" }).text, /assessor's photo of 1 Example Street/);
  assert.equal(peek({ touched: { ...lot, facts: { name: "2 Example Street" } }, reason: "touch" }), null);
  assert.equal(peek({ touched: lot, reason: "near" }), null);
});

test("the gallery offers a showcase shot when you're near where it was taken", async () => {
  const sc = JSON.parse(readFileSync(new URL("showcase.json", SITE_DIR), "utf8"));
  const realFetch = globalThis.fetch;
  globalThis.location = { href: "http://localhost/ghent/" };
  globalThis.fetch = async () => ({ ok: true, json: async () => sc });
  try {
    const { peek } = await import(moduleUrl(site.renderers.gallery));
    const s = sc.shots[0];
    const ctx = (x, z, reason = "near") => ({ reason, showcase: "/sites/ghent-streets/showcase.json", here: { x, z } });
    const got = await peek(ctx(s.stand[0] + 3, s.stand[1] - 2));
    assert.equal(got.key, `gallery:${s.id}`);
    assert.ok(got.text.startsWith(s.title), "it names the shot");
    assert.equal(typeof got.open, "function");
    assert.equal(await peek(ctx(s.stand[0] + 3, s.stand[1] - 2, "touch")), null, "only as you go, not on a touch");
    const far = await peek(ctx(s.stand[0] + 5000, s.stand[1] + 5000));
    assert.equal(far, null, "nothing far from every shot");
  } finally {
    globalThis.fetch = realFetch;
    delete globalThis.location;
  }
});
