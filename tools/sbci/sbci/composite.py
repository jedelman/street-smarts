"""One optional single-number lens on SCCI/SCED/SEI — not the report.

survey.py is the primary output of this package: three metrics kept
separate, disagreements between them surfaced as questions rather than
folded away. This module exists because a single number is occasionally
useful for a quick sort or a shared-color-scale map, not because it's an
answer. Don't cite sbci as if it settles anything a look at the three
sub-metrics wouldn't settle better — see the package README's note on
this repo's own anti-measurement stance.

The brief doesn't specify a compositing formula beyond naming the three
inputs, so this makes one explicit choice and documents it rather than
leaving it implicit: SBCI reads as "how much of this cell's spatial
character comes from commons/porosity vs. fixed capital," so it's built
as a ratio, not a sum — SCCI pulls it down, SCED and SEI pull it up. A
sum would conflate "everything is high" with "commons wins," which is
the entire distinction the brief cares about.

  sbci = (sced_norm + sei_norm) / (1 + scci_norm)

Range is (0, 2] — not normalized to [0,1] across cities, deliberately:
squashing both cities into a shared 0..1 range would hide the fact that
Norfolk and Barcelona have genuinely different absolute SCCI/SCED/SEI
distributions, which is the entire point of the two-city comparison. Do
cross-city ranking on the raw sub-metrics (scci_norm, sced_norm, sei_norm
are each already normalized within their own city's grid), not on sbci
directly, unless you've decided a shared 0..2 scale is actually meaningful
for your question.
"""
from __future__ import annotations

import geopandas as gpd


def compute_sbci(grid_with_metrics: gpd.GeoDataFrame) -> gpd.GeoDataFrame:
    required = {"scci_norm", "sced_norm", "sei_norm"}
    missing = required - set(grid_with_metrics.columns)
    if missing:
        raise ValueError(
            f"missing columns {missing} — run compute_scci, compute_sced, "
            "compute_sei first and merge their outputs on cell_id"
        )

    out = grid_with_metrics.copy()
    out["sbci"] = (out["sced_norm"] + out["sei_norm"]) / (1 + out["scci_norm"])
    return out
