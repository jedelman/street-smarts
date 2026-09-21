# sbci — a survey instrument, not a pitch

A standalone Python pipeline computing three separate spatial metrics on a
100m grid over Norfolk, VA and Barcelona, Spain, and — deliberately — never
collapsing them into one verdict:

- **SCCI** (Spatial Constant Capital Intensity) — built-volume / impervious-surface
  density per cell, from OSM building footprints + street network.
- **SCED** (Spatial Commons & ESS Density) — Gaussian-KDE density of cooperative,
  commons, and solidarity-economy nodes (1km bandwidth), weighted by governance tier.
- **SEI** (Network Porosity & Enclosure Index) — pedestrian-network alpha/gamma
  circuit indices minus physical-boundary enclosures (fences, rail corridors,
  military buffers, highway barriers).

**The headline output is `sbci/survey.py`, not a composite score.** It keeps
the three metrics separate, flags cells where they disagree with each other
in specific, meaningful ways, and phrases each one as a question for a human
to go check — not a conclusion. It also distinguishes "we measured this and
found little" from "we have no data here" (an *abstention*), so a missing
ESS layer for Norfolk can't silently read as "Norfolk has no commons." A
single composite `sbci` number still exists in `composite.py`, for the rare
case a quick sort or a shared color scale is actually useful — but nothing
in this package treats it as the report.

## Relationship to street-smarts' Rust/WASM core

This tool is **not wired into** the `street-smarts-*` crates or the WASM build.
It's a separate, independently-runnable Python module living under `tools/`
(same pattern as `tools/canopy-loss`), because the analysis technique (grid
KDE over fetched geodata) has nothing in common with the NIR/opinion pipeline
in machinery. The *posture* is deliberately the same one, though:
`street-smarts-conflict`'s own doc comment says disagreement is the primary
product, not something to resolve — it surfaces `questions_for_humans` and
`abstentions` rather than a single score. `sbci/survey.py` is that same idea
applied to continuous metrics instead of discrete Alexander-pattern opinions.

This repo's top-level README states the premise directly: *"This is not a
measurement tool. It does not claim to measure aliveness... Anyone selling
you a number is selling you a deck."* A composite SBCI score is, structurally,
exactly that kind of number — which is why `survey.py`, not `composite.py`,
is what this package actually hands back. Treat any single metric here the
way the brief's own Step 2 weights are explicitly *opinions encoded as
numbers* (co-op housing = 1.0, standard commercial = 0.0): useful for
provoking a specific look, never a validated measurement of anything. Don't
cite an sbci value, or any one of SCCI/SCED/SEI alone, as if it settles a
question the way a Census figure would.

## What's actually live vs. stubbed

Tested from this sandbox's network (a proxied, policy-restricted egress):

| Source | Status | Notes |
|---|---|---|
| OSM Overpass API (via `osmnx`) | **Blocked here** | Connection reset by the sandbox's egress proxy on every attempt (`overpass-api.de:443`, `ws_closed_mid_exchange`). Code is real and will work in an unrestricted environment — `osm_source.py` has no synthetic fallback, it just raises. |
| Nominatim (geocoding) | Live, tested (200) | Used only for bbox lookup by place name. |
| US Census ACS API | Code is live-real, **needs an API key** | `https://api.census.gov/...` returns "Missing Key" without one. Set `CENSUS_API_KEY`. Get one free at https://api.census.gov/data/key_signup.html. |
| Open Data BCN (CKAN) | Live, tested — `find_dataset_resources` returns real results | Tested with `q="urbanisme"` (122 hits) and `q="transport"` (114 hits); `q="superilles"` returned 0 — that keyword doesn't match this catalog's indexing, so finding the actual superblocks/casals-de-barri dataset needs a better search term or browsing the portal, not a code fix. |
| Catastro (Sede Electrónica) | **Stubbed** | SOAP/XML service with its own quirks (parcel-by-parcel lookup, no bulk bbox query in the free tier). `catastro_source.py` documents the real endpoint and raises `NotImplementedError` rather than faking floor-area data. |
| XES / Pam a Pam ESS directories | **Stubbed with a small hand-seeded fixture** | No public bulk API found. `data/ess_nodes_barcelona_seed.geojson` has a handful of well-known, real cooperatives (La Borda, Can Batlló, Coop57) with approximate coordinates — flagged `"verified": false` per point. Do not treat as a complete or precisely-geocoded ESS census. Replace with a real Pam a Pam export before drawing conclusions. |

Every network call in this package fails loudly (raises) instead of silently
returning fabricated numbers. There is no synthetic-data fallback mode —
if a source is unreachable, the pipeline stops there and tells you which
stage failed.

## Layout

```
sbci/
  grid.py            100m vector grid construction over a bbox (pure geometry, no network)
  osm_source.py       osmnx wrappers: street network, building footprints
  census_source.py    ACS block-group pulls (median income, commute mode, vehicle ownership)
  opendata_bcn.py      Open Data BCN CKAN client (municipal facilities, superblocks)
  catastro_source.py  STUB — Catastro Sede Electrónica interface, not implemented
  ess_source.py        ESS node loader (GeoJSON in, category/weight normalization)
  scci.py              Step 1: Spatial Constant Capital Intensity
  sced.py               Step 2: Spatial Commons & ESS Density (Gaussian KDE)
  sei.py                Step 3: Network Porosity & Enclosure Index
  survey.py             PRIMARY OUTPUT — chorus summary, abstentions, questions_for_humans
  composite.py          optional single-number sbci lens — not the report, see survey.py
  pipeline.py           run_spatial_capital_analysis() orchestrator
  viz.py                Dual-panel heatmaps (matplotlib), folium isochrone overlay
tests/                 Unit tests against synthetic geometry — no network calls
examples/               run_norfolk.py / run_barcelona.py entry points
data/                   ESS seed fixture (see table above)
```

## Setup

```bash
cd tools/sbci
pip install -r requirements.txt
export CENSUS_API_KEY=...   # optional, only needed for census_source.py
```

## Running

```bash
python examples/run_norfolk.py     # will fail at the OSM fetch step in a
                                    # network-restricted sandbox; works
                                    # wherever Overpass is reachable. Prints
                                    # a survey report (chorus + questions),
                                    # not a single verdict.
python -m pytest tests/            # network-free, exercises real formulas
                                    # (including survey.py's question logic)
                                    # against synthetic fixtures — 14 tests
```
