"""Step 3: Network Porosity & Enclosure Index (SEI).

Per grid cell:
  1. node density: pedestrian-network intersection nodes per cell area
  2. alpha index (planar graph circuit index):
       alpha = (e - v + 1) / (2v - 5)     for v >= 3
     measures redundancy / route choice — 0 = tree (no alternate routes),
     1 = maximally connected planar graph
  3. gamma index: e / (3 * (v - 2))       for v >= 3
     ratio of actual to maximum possible edges in a planar graph
  4. enclosure penalty: fraction of cell boundary length coincident with
     barrier features (fences, rail corridors, military-buffer polygons,
     motorway/trunk roads treated as barriers, not thoroughfares)

  sei_raw = porosity_component - enclosure_penalty
  porosity_component = mean(node_density_norm, alpha, gamma)   [alpha/gamma
  are already 0..1 by construction; node_density is min-max normalized
  across the grid]

alpha/gamma are undefined (division degenerate or meaningless) below v=3
nodes in a cell — those cells get alpha=gamma=0 rather than NaN, since a
near-empty local subgraph has by construction no circuit redundancy.
"""
from __future__ import annotations

import geopandas as gpd
import networkx as nx
import numpy as np
from shapely.geometry import Point


BARRIER_HIGHWAY_TYPES = {"motorway", "trunk", "motorway_link", "trunk_link"}


def _local_subgraph_stats(G: nx.MultiDiGraph, cell_geom) -> tuple[int, int]:
    """Return (v, e) for the subgraph of G induced by nodes falling inside
    cell_geom. Uses node point-in-polygon, not edge clipping — an edge
    that crosses the cell boundary without either endpoint inside is not
    counted, which undercounts porosity for very small cells relative to
    typical block length. Documented limitation, not silently absorbed."""
    nodes_in_cell = [
        n
        for n, data in G.nodes(data=True)
        if cell_geom.contains(Point(data["x"], data["y"]))
    ]
    if not nodes_in_cell:
        return 0, 0
    sub = G.subgraph(nodes_in_cell)
    return sub.number_of_nodes(), sub.number_of_edges()


def compute_sei(
    grid: gpd.GeoDataFrame,
    pedestrian_graph: nx.MultiDiGraph,
    barrier_features: gpd.GeoDataFrame | None = None,
) -> gpd.GeoDataFrame:
    """
    pedestrian_graph : osmnx graph (network_type='walk'), already projected
        to grid.crs by the caller — osmnx graphs carry CRS in graph attrs,
        check with ox.projection.is_projected(G.graph['crs']) before calling.
    barrier_features : optional GeoDataFrame of linear/polygon barriers
        (fences, rail, military buffers) in grid.crs; motorway/trunk edges
        from pedestrian_graph's parent drive network should be passed here
        by the caller if barrier-highway exclusion is wanted, since the
        walk-network graph itself won't contain them.
    """
    out = grid.copy()
    v_list, e_list = [], []
    for geom in out.geometry:
        v, e = _local_subgraph_stats(pedestrian_graph, geom)
        v_list.append(v)
        e_list.append(e)
    out["node_count"] = v_list
    out["edge_count"] = e_list

    out["node_density"] = out["node_count"] / out["cell_area_m2"]
    max_density = out["node_density"].max()
    out["node_density_norm"] = (
        out["node_density"] / max_density if max_density > 0 else 0.0
    )

    alpha, gamma = [], []
    for v, e in zip(out["node_count"], out["edge_count"]):
        if v >= 3:
            denom_a = 2 * v - 5
            denom_g = 3 * (v - 2)
            alpha.append(max(0.0, min(1.0, (e - v + 1) / denom_a)) if denom_a > 0 else 0.0)
            gamma.append(max(0.0, min(1.0, e / denom_g)) if denom_g > 0 else 0.0)
        else:
            alpha.append(0.0)
            gamma.append(0.0)
    out["alpha_index"] = alpha
    out["gamma_index"] = gamma

    out["porosity_component"] = out[
        ["node_density_norm", "alpha_index", "gamma_index"]
    ].mean(axis=1)

    if barrier_features is not None and len(barrier_features):
        if barrier_features.crs != grid.crs:
            raise ValueError(
                f"CRS mismatch: grid={grid.crs}, barriers={barrier_features.crs}"
            )
        penalties = []
        for geom in out.geometry:
            boundary_len = geom.boundary.length
            if boundary_len == 0:
                penalties.append(0.0)
                continue
            intersecting = barrier_features[barrier_features.intersects(geom)]
            if intersecting.empty:
                penalties.append(0.0)
                continue
            barrier_len = intersecting.geometry.intersection(geom.boundary).length.sum()
            penalties.append(min(1.0, barrier_len / boundary_len))
        out["enclosure_penalty"] = penalties
    else:
        out["enclosure_penalty"] = 0.0

    out["sei_raw"] = out["porosity_component"] - out["enclosure_penalty"]
    # sei_raw is already bounded [-1, 1] by construction (porosity in [0,1],
    # penalty in [0,1]); normalize to [0,1] for compositing with SCCI/SCED
    out["sei_norm"] = (out["sei_raw"] + 1) / 2
    return out
