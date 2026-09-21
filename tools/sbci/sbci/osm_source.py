"""OSM street network + building footprint fetches via osmnx.

No synthetic fallback: if Overpass is unreachable, these raise. Confirmed
blocked from the dev sandbox this pipeline was authored in (connection reset
on overpass-api.de:443) — see the README's live/stubbed table. Real code,
tested against real OSM data structure, just not exercised end-to-end here.
"""
from __future__ import annotations

import geopandas as gpd
import osmnx as ox

# Tags kept for building-footprint feature engineering per the brief's
# "Building Footprints & Heights" row: footprint area is direct from
# geometry; height/levels come through when OSM contributors tagged them
# (`building:levels`), which is patchy coverage — treat missing levels as
# unknown, not zero.
BUILDING_TAGS = {"building": True}


def fetch_street_network(
    bbox_wgs84: tuple[float, float, float, float], network_type: str = "all"
) -> "ox.graph":
    """Pedestrian + drive network within bbox. network_type='walk' for the
    SEI porosity calc specifically; 'all' for general SCCI infrastructure
    density."""
    min_lon, min_lat, max_lon, max_lat = bbox_wgs84
    return ox.graph_from_bbox(
        bbox=(max_lat, min_lat, max_lon, min_lon), network_type=network_type
    )


def fetch_building_footprints(
    bbox_wgs84: tuple[float, float, float, float],
) -> gpd.GeoDataFrame:
    """Building footprint polygons with whatever land-use/height tags OSM has."""
    min_lon, min_lat, max_lon, max_lat = bbox_wgs84
    gdf = ox.features_from_bbox(
        bbox=(max_lat, min_lat, max_lon, min_lon), tags=BUILDING_TAGS
    )
    gdf = gdf[gdf.geometry.type.isin(["Polygon", "MultiPolygon"])].copy()
    gdf["footprint_area_m2"] = gdf.to_crs(gdf.estimate_utm_crs()).geometry.area
    if "building:levels" in gdf.columns:
        gdf["levels"] = pd_to_numeric_safe(gdf["building:levels"])
    else:
        gdf["levels"] = None
    return gdf


def pd_to_numeric_safe(series):
    import pandas as pd

    return pd.to_numeric(series, errors="coerce")
