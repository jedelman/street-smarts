# Notes for Jason — what Claude did overnight

## Status: skeleton built, WASM compiled, repo ready to push and deploy

The Rust workspace is on disk at `/tmp/street-smarts` and pushed to GitHub. The 242 KB WASM bundle is in `public/wasm/`. `wrangler deploy --dry-run` passes — packages 12 files cleanly.

**Not yet deployed** because I don't have a Cloudflare API token in this environment. Wrangler can't run `wrangler deploy` (non-dry-run) without one. Two options for you when you're back:

1. From your laptop:
   ```bash
   git clone git@github.com:jedelman/street-smarts.git
   cd street-smarts
   wrangler deploy
   ```
   You're already authenticated locally (you deploy jason-edelman.org from there). This should take ~30 seconds and give you a `street-smarts.<your-subdomain>.workers.dev` URL.

2. Give me a scoped CF API token (Workers Scripts:Edit on this one account) and I can deploy from here.

I went with option 1's preparation because it doesn't require you to mint a new token tonight.

## What's actually in the build

### Three opinions that produce honest, meaningfully different outputs

The Rust integration test against the EC fixtures showed:

**Baseline (188 existing parcels, Military Circle as-is):**
- Levels of Scale: **1.00** — 7 levels, ratios 2.6× / 3.1× / 2.4× / 2.9× / 3.9× / 2.3×. All in Salingaros's 2–5× band. The existing parcel geometry is actually well-scaled; Norfolk's mid-century fabric isn't dead in the way the buildings are.
- Strong Centers: **no view** — no activity nodes, plazas, or named landmarks tagged in the baseline. Honest abstention.
- Ownership: **no view** — no ownership tags in the baseline data. Honest abstention.

**Proposal (207 parcels including 19 EDA parcels = the EC proposal):**
- Levels of Scale: **1.00** — basically unchanged. The proposal doesn't *destroy* the existing scale variety, which is good news (some master-plan tools would).
- Strong Centers: **0.93** — 16 named centers (CLT_GLENROCK, CLT_NORTH, CIVIC_700, CIVIC_920, MAIN_ST_*, MALL_CORE). 926× weight hierarchy. Mean nearest-neighbor 83m, 3.8% of bbox diagonal — a bit clustered.
- Ownership: **1.00** — 100% of tagged land in commons / CLT / civic. The activist axis loves this proposal.

**The story the chorus tells:** the baseline already has scale variety (interesting and a little surprising). What the EC proposal *adds* is centers and ownership — exactly what the coalition claims it adds. The library independently confirms the proposal does what it says it does on these axes.

### What the conflict engine surfaced

On the proposal, the conflict engine's headline reads:

> "The Geometric chorus is in rough agreement around 0.96."
> "The Activist chorus is in rough agreement around 1.00."

Top question:

> "All voices roughly agree. What do you see that the algorithms might be missing?"

This is the *right* prompt in this case — agreement among three opinions that share many assumptions is suspicious, and the question redirects to the human.

On the baseline:

> "Levels of Scale has a view here; Strong Centers couldn't see enough data to speak. Where would you say the centers are?"

Honest about what the algorithm can't see. The prompt actually invites human contribution rather than faking a number.

### Decisions I made unilaterally — flag for your review

1. **License conflict.** The existing LICENSE file in the repo (from your initial commit) is MIT. The README I wrote and the workspace Cargo.toml both declare AGPL-3.0-or-later (matching the spec). I did NOT touch the LICENSE file. **Tell me which you want and I'll align everything.** Spec recommendation: AGPL-3.0 anti-enclosure. If you want to keep MIT, that's also fine but it gives developers a free hand to use this without giving back.

2. **The conflict engine v0.1 has weak disagreement.** Because each axis has exactly one opinion, there's no within-axis disagreement to surface. The interesting disagreement is between *baseline* and *proposal*, but I didn't build a comparison-mode renderer tonight. The UI shows them as two tabs instead. v0.2 should add proper diff rendering.

3. **No tests for the WASM layer.** The Rust core has a unit test and an integration test (both pass). The WASM bridge is untested. Easy to fix once you have a deployed URL to point at.

4. **ferrotorch is not yet a dependency.** v0.1 has no neural-net opinions, so I didn't link it. The architecture is ready for it — `street-smarts-opinions` can add a `geometric/vit_*` module that depends on `ferrotorch-vision` without disrupting anything else. Tonight would have been the wrong time to wrestle with WASM build constraints on a large dep tree.

5. **Equity guards aren't enforced in the generator** because there is no generator in v0.1. The activist opinion is just *displayed* separately, not used to gate anything. v0.2's generator wires this up properly.

### Honest list of things that would catch fire under hostile review

- **Single fixture, single test.** Three opinions on Eastside Commons isn't enough to know if the algorithms are right. Need Siena, Trastevere, Tysons Corner pre-2010 as sanity-floor sites before v0.2.
- **Ownership opinion treats EDA-tagged parcels as commons-aspirational.** This is *interpretation*, not ownership data. The opinion's caveats say so loudly, but a reviewer could fairly say the opinion is begging the question.
- **Levels of Scale only looks at parcel + building footprints + open space areas.** A neighborhood with one parcel size but lots of building variety would score badly. The caveats say so. Future opinions should look at building height variety, façade detail, street-tree spacing.
- **The conflict engine's "all agree" prompt** is a fine fallback but it's also what the engine says when the algorithms genuinely failed and agreed by accident. v0.2 should distinguish.
- **Bundle is 242KB unminified WASM** because wasm-opt couldn't be installed (network restricted in this sandbox). On your laptop, running wasm-pack with wasm-opt available will likely shrink to 80–120KB. Not urgent.
- **No accessibility audit.** The HTML is semantic and uses real headings, but I didn't run axe-core. Probably fine, definitely should be verified.

### What's in the GitHub commit

Pushed to `github.com/jedelman/street-smarts`:

```
Cargo.toml + Cargo.lock              workspace config
crates/                              6 Rust crates
public/                              static deployment artifacts
  index.html                         the page
  style.css                          restrained styling
  app.js                             frontend
  eastside-baseline.json             188 parcels, no ownership tags
  eastside-proposal.json             207 parcels with EDA proposal
  wasm/                              built WASM bundle
data/                                NIR fixtures (also copied into public/)
scripts/convert-ec-data.js           EC parcel-data → NIR converter
SPEC.md                              v0.2 spec
NOTES.md                             this file
wrangler.toml                        Cloudflare Workers Assets config
README.md                            the README
.gitignore
LICENSE                              MIT — flagged above
```

### Questions to answer when you wake up

1. **License?** MIT (existing LICENSE) or AGPL-3.0 (README/Cargo)?
2. **Deploy.** Want to deploy from your laptop or hand me a scoped token?
3. **Once deployed and live, does the page actually read right on your phone?** I built it mobile-first but I have not held a real phone to it.
4. **Anything in the chorus's framing feel wrong?** Specifically: "What to argue about" prompts, opinion-card layout, the "what the algorithms refused to speak to" section. These are the v0.1 distinctive UX moves — if they feel off, they're easier to change now than after a coalition meeting goes sideways.

### What's already on the slate for v0.2

- Remaining 7 geometric opinions (Thick Boundaries, Positive Space, Local Symmetries, Echoes, The Void, Not-Separateness, and 6 more — most of these have clean classical algorithms)
- ~40 pattern-presence opinions (port from your existing EC pattern detectors)
- 4 more activist opinions (affordability, displacement, ecology, mobility — these need real data sources, not just NIR)
- Steerable generator wired to opinion gradients (ferrotorch lands here)
- Comparison mode (baseline ↔ proposal diff card)
- Decision ledger (IndexedDB, opt-in, exportable, deletable)
- Direct manipulation UI (tap to add a plaza, drag a building, redraw a street)
- Multiple-opinions-per-axis to make the conflict engine sing

Spec v0.3 will reflect all of this and will land before v0.2 implementation starts.

---

That's the honest status. The skeleton works. The chorus actually talks. The path from here to a live URL is one `wrangler deploy` away.

Get some more sleep.
