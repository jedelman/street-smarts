# street-smarts

A Christopher Alexander **provocation engine for neighborhood imagination**.

> *"What's accurate is the record of what people decided. Everything else is opinion."*

This is not a measurement tool. It does not claim to measure aliveness — nobody can. Aliveness is what Alexander called the *quality without a name*: by definition not fully namable. Anyone selling you a number is selling you a deck.

What this library does instead:

- **Provokes.** Runs neighborhood data through Alexander's corpora (15 properties of wholeness, 253 patterns, ordered sequences) encoded as opinions in math.
- **Disagrees.** Multiple opinions look at the same place from different angles. Their disagreements are the primary output.
- **Imagines.** (v0.2+) Lets people apply pattern operators to dead places and see what comes out.
- **Records.** (v0.2+) Tracks who looked at what, who said what, what got decided afterward. The decision ledger is the data.

This is a tool for **activists and neighborhood groups**, not planners, not developers. It runs in a browser — no cloud account, no API keys, no Big Tech dependencies in the user path.

## Status: v0.1

A working skeleton. Three opinions in the chorus, evaluated against the Eastside Commons proposal (73 acres, Military Circle, Norfolk):

- **Levels of Scale** (geometric, per Salingaros 2025) — does this place have variety of feature sizes, with ratios in the 2–5× band?
- **Strong Centers** (geometric, per Salingaros 2025) — does this place have a hierarchy of named centers, spread across it?
- **Ownership Pattern** (activist, per Edelman / Eastside Commons coalition) — who owns this land?

## Architecture

```
crates/
├── street-smarts-core/         NIR schema, geometry primitives, opinion protocol
├── street-smarts-opinions/     Concrete opinion implementations
├── street-smarts-conflict/     Disagreement detection, human prompts
├── street-smarts-ledger/       Decision ledger types (stub in v0.1)
├── street-smarts-generator/    Pattern operators, steering loop (stub in v0.1)
└── street-smarts-web/          WASM bindings + Cloudflare Worker target

public/                         Static assets — HTML, CSS, JS, fixtures, WASM
data/                           NIR fixtures (Eastside Commons baseline + proposal)
scripts/                        Helpers (EC parcel-data → NIR converter)
```

The substrate is Rust + WebAssembly. Compiles via `wasm-pack` and deploys via Cloudflare Workers Assets.

v0.2 will introduce [ferrotorch](https://github.com/forecast-bio/ferrotorch) for the steerable generator and neural-net-based geometric opinions (ViT/ConvNeXt vision opinions in pure Rust, runnable in WASM via CubeCL).

## Build & run locally

```bash
# install rust + wasm32 target
rustup target add wasm32-unknown-unknown

# build the WASM bundle
wasm-pack build crates/street-smarts-web --target web --release --out-dir ../../public/wasm

# regenerate fixtures (only if EC parcel data changes)
node scripts/convert-ec-data.js \
    /path/to/jason-edelman.org/eastside-commons/ec-parcel-data.js \
    data/eastside-baseline.json data/eastside-proposal.json
cp data/*.json public/

# serve
cd public && python3 -m http.server 8000
```

## Deploy

Workers Assets static deploy:

```bash
wrangler deploy             # → street-smarts.<your-subdomain>.workers.dev
```

The wrangler.toml is preconfigured for the jedelman Cloudflare account.

## License

AGPL-3.0-or-later. Anti-enclosure license: if a city or a developer uses this, they have to give back.

## What this is not

- Not a measurement tool.
- Not a planner's tool.
- Not a developer's tool.
- Not a "smart cities" platform.
- Not cloud-dependent in the activist path.

## What this is

A small library that helps people imagine concretely what they want where they live, and argue clearly about it.

It will not stop the next REIT. But every coalition meeting where someone says *"here's what we want instead, here are five things to argue about, here's what we decided together"* is one inch of ground the enclosure does not get.

---

Specification: see `SPEC.md` for the v0.2 spec; v0.3 forthcoming. Earlier drafts in [jedelman/claude-memory](https://github.com/jedelman/claude-memory) `conversations/2026-05-17-*`.

Reference: Salingaros, N.A. *Living geometry, AI tools, and Alexander's 15 fundamental properties.* Frontiers of Architectural Research 14(6): 1491–1515, 2025. CC BY-NC-ND. DOI [10.1016/j.foar.2025.01.002](https://doi.org/10.1016/j.foar.2025.01.002).
