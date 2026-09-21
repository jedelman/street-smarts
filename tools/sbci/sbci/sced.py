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

    centroids = grid.geometry.centroid
    cx = centroids.x.to_numpy()
    cy = centroids.y.to_numpy()

    node_x = ess_nodes.geometry.x.to_numpy()
    node_y = ess_nodes.geometry.y.to_numpy()
    node_w = ess_nodes["weight"].to_numpy()

    if len(node_x) == 0:
        out = grid.copy()
        out["sced_raw"] = 0.0
        out["sced_norm"] = 0.0
        return out

    # pairwise distances: (n_cells, n_nodes) — fine at city-grid scale
    # (tens of thousands of cells x low hundreds of nodes); would need a
    # KD-tree cutoff for much larger inputs.
    dx = cx[:, None] - node_x[None, :]
    dy = cy[:, None] - node_y[None, :]
    dist2 = dx**2 + dy**2

    kernel = np.exp(-dist2 / (2 * bandwidth_m**2))
    sced_raw = (kernel * node_w[None, :]).sum(axis=1)

    out = grid.copy()
    out["sced_raw"] = sced_raw
    max_raw = out["sced_raw"].max()
    out["sced_norm"] = out["sced_raw"] / max_raw if max_raw > 0 else 0.0
    return out
