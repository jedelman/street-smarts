"""Step 2: Spatial Commons & ESS Density (SCED).

Gaussian KDE of weighted ESS node locations, evaluated at each grid cell
centroid, bandwidth R=1000m per the brief.

Gaussian kernel: K(d) = exp(-d^2 / (2*R^2))
SCED(cell) = sum_j( weight_j * K(dist(cell_centroid, node_j)) )

This is a direct weighted-sum KDE (not scikit-learn's KernelDensity, which
normalizes to a probability density and doesn't take per-point weights the
way we need here) — implemented directly so the weights in the brief's
Step 2 table map onto the sum term exactly, not through a library's
density normalization.

Abstention, not silent zero: an empty ess_nodes layer produces sced_raw=0
everywhere, which is indistinguishable in the numbers from "we looked and
found no commons here." Those are different claims — the first is "we
haven't collected this layer," the second is a real measurement. This
module marks the former with sced_abstain=True on every cell, echoing
street-smarts-conflict's Abstention concept (an opinion that declined to
speak, with a reason) rather than letting an empty input read as a
confident negative result.
"""
from __future__ import annotations

import geopandas as gpd
import numpy as np

BANDWIDTH_M = 1000.0


def compute_sced(
    grid: gpd.GeoDataFrame, ess_nodes: gpd.GeoDataFrame, bandwidth_m: float = BANDWIDTH_M
) -> gpd.GeoDataFrame:
    if ess_nodes.crs != grid.crs:
        raise ValueError(
            f"CRS mismatch: grid={grid.crs}, ess_nodes={ess_nodes.crs} — "
            "reproject before calling compute_sced"
        )

    out = grid.copy()

    if len(ess_nodes) == 0:
        out["sced_raw"] = 0.0
        out["sced_norm"] = 0.0
        out["sced_abstain"] = True
        out["sced_abstain_reason"] = "ess_nodes layer is empty — no data collected, not a measured absence"
        return out

    centroids = grid.geometry.centroid
    cx = centroids.x.to_numpy()
    cy = centroids.y.to_numpy()

    node_x = ess_nodes.geometry.x.to_numpy()
    node_y = ess_nodes.geometry.y.to_numpy()
    node_w = ess_nodes["weight"].to_numpy()

    # pairwise distances: (n_cells, n_nodes) — fine at city-grid scale
    # (tens of thousands of cells x low hundreds of nodes); would need a
    # KD-tree cutoff for much larger inputs.
    dx = cx[:, None] - node_x[None, :]
    dy = cy[:, None] - node_y[None, :]
    dist2 = dx**2 + dy**2

    kernel = np.exp(-dist2 / (2 * bandwidth_m**2))
    sced_raw = (kernel * node_w[None, :]).sum(axis=1)

    out["sced_raw"] = sced_raw
    max_raw = out["sced_raw"].max()
    out["sced_norm"] = out["sced_raw"] / max_raw if max_raw > 0 else 0.0
    out["sced_abstain"] = False
    out["sced_abstain_reason"] = None

    if "verified" in ess_nodes.columns and not ess_nodes["verified"].all():
        n_unverified = int((~ess_nodes["verified"]).sum())
        out["sced_confidence_note"] = (
            f"{n_unverified}/{len(ess_nodes)} contributing nodes have "
            "unverified/approximate coordinates — treat sced_raw as directional, "
            "not precise, in this run."
        )
    else:
        out["sced_confidence_note"] = None

    return out
