"""Step 1: Spatial Constant Capital Intensity (SCCI).

SCCI approximates the "dead labor" embedded in fixed spatial infrastructure
per cell — built volume and paved/impervious surface, normalized by cell
area. Where OSM `building:levels` coverage exists we use footprint * levels
as a volume proxy; where it doesn't (most of the world), footprint-only
coverage ratio is the fallback and is labeled as such in the output.
"""
from __future__ import annotations

import geopandas as gpd
import numpy as np
import pandas as pd


def compute_scci(
    grid: gpd.GeoDataFrame,
    buildings: gpd.GeoDataFrame,
    road_network_edges: gpd.GeoDataFrame | None = None,
) -> gpd.GeoDataFrame:
    """Join buildings (and optionally road edges, as an impervious-surface
    proxy) onto grid cells, return grid with scci_raw / scci_norm columns.

    Parameters
    ----------
    grid : output of sbci.grid.build_grid, must share buildings' CRS
    buildings : output of sbci.osm_source.fetch_building_footprints,
        reprojected to grid.crs by the caller (pipeline.py does this)
    road_network_edges : optional edges GeoDataFrame (from osmnx graph, via
        ox.graph_to_gdfs) for a paved-surface density term
    """
    if buildings.crs != grid.crs:
        raise ValueError(
            f"CRS mismatch: grid={grid.crs}, buildings={buildings.crs} — "
            "reproject before calling compute_scci"
        )

    joined = gpd.sjoin(
        buildings[["geometry", "footprint_area_m2", "levels"]],
        grid[["cell_id", "geometry"]],
        how="inner",
        predicate="intersects",
    )

    has_levels = joined["levels"].notna()
    joined["volume_proxy_m3"] = np.where(
        has_levels,
        joined["footprint_area_m2"] * joined["levels"] * 3.0,  # 3m/story
        joined["footprint_area_m2"],  # footprint-only fallback
    )

    per_cell = joined.groupby("cell_id").agg(
        building_footprint_m2=("footprint_area_m2", "sum"),
        volume_proxy_m3=("volume_proxy_m3", "sum"),
        buildings_with_height_data=("levels", lambda s: s.notna().sum()),
        building_count=("footprint_area_m2", "count"),
    )

    out = grid.merge(per_cell, on="cell_id", how="left")
    for col in [
        "building_footprint_m2",
        "volume_proxy_m3",
        "buildings_with_height_data",
        "building_count",
    ]:
        out[col] = out[col].fillna(0.0)

    out["coverage_ratio"] = (out["building_footprint_m2"] / out["cell_area_m2"]).clip(
        upper=1.0
    )

    if road_network_edges is not None and len(road_network_edges):
        if road_network_edges.crs != grid.crs:
            raise ValueError(
                f"CRS mismatch: grid={grid.crs}, roads={road_network_edges.crs}"
            )
        # buffer road centerlines by a nominal half-width to approximate
        # paved surface area; osmnx doesn't give us real pavement width per
        # edge, so this is a coarse proxy, documented as such in the column.
        buffered = road_network_edges.copy()
        buffered["geometry"] = buffered.geometry.buffer(3.0)  # ~6m paved width
        road_joined = gpd.overlay(
            buffered[["geometry"]], grid[["cell_id", "geometry"]], how="intersection"
        )
        road_joined["paved_area_m2"] = road_joined.geometry.area
        paved_per_cell = road_joined.groupby("cell_id")["paved_area_m2"].sum()
        out = out.merge(paved_per_cell, on="cell_id", how="left")
        out["paved_area_m2"] = out["paved_area_m2"].fillna(0.0)
        out["impervious_ratio_proxy"] = (
            (out["building_footprint_m2"] + out["paved_area_m2"]) / out["cell_area_m2"]
        ).clip(upper=1.0)
    else:
        out["paved_area_m2"] = np.nan
        out["impervious_ratio_proxy"] = out["coverage_ratio"]

    out["scci_raw"] = out["volume_proxy_m3"] / out["cell_area_m2"]
    max_raw = out["scci_raw"].max()
    out["scci_norm"] = out["scci_raw"] / max_raw if max_raw > 0 else 0.0

    return out
