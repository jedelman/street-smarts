"""ESS (Economia Social i Solidària) node loading + governance-tier weights.

Per the brief's Step 2 weights:
  co-op housing / CLT            -> 1.0
  worker co-op / ethical finance -> 0.8
  public-cooperative facility    -> 0.7
  standard commercial            -> 0.0

No bulk public API was found for XES's directory or Pam a Pam during this
session (both appear to be browse-only web directories, not open-data
endpoints) — see data/ess_nodes_barcelona_seed.geojson's disclaimer. This
module just loads whatever GeoJSON it's given and normalizes categories to
weights; it doesn't know or care whether the source is the seed fixture or
a real export.
"""
from __future__ import annotations

import geopandas as gpd

CATEGORY_WEIGHTS = {
    "coop_housing_clt": 1.0,
    "worker_coop": 0.8,
    "worker_coop_ethical_finance": 0.8,
    "public_cooperative_facility": 0.7,
    "standard_commercial": 0.0,
}


def load_ess_nodes(geojson_path: str) -> gpd.GeoDataFrame:
    gdf = gpd.read_file(geojson_path)
    if gdf.empty:
        gdf["category"] = []
        gdf["weight"] = []
        gdf["verified"] = []
        return gdf
    if "category" not in gdf.columns:
        raise ValueError(f"{geojson_path}: features missing 'category' property")

    unknown = set(gdf["category"]) - set(CATEGORY_WEIGHTS)
    if unknown:
        raise ValueError(f"{geojson_path}: unrecognized categories {unknown}")

    if "weight" not in gdf.columns:
        gdf["weight"] = gdf["category"].map(CATEGORY_WEIGHTS)
    else:
        # trust an explicit per-feature weight if given, fill gaps from the
        # category default
        gdf["weight"] = gdf["weight"].fillna(gdf["category"].map(CATEGORY_WEIGHTS))

    if "verified" not in gdf.columns:
        gdf["verified"] = False

    return gdf
