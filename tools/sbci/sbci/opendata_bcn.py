"""Open Data BCN (Ajuntament de Barcelona) CKAN API client.

Live endpoint, no key required for public datasets (tested 200 from this
sandbox: opendata-ajuntament.barcelona.cat/data/api/3/action/status_show).
CKAN's package_search is used to locate the current dataset ids for
superblocks (superilles) and civic centers (casals de barri) — those ids
are not hardcoded because CKAN dataset slugs on this portal have changed
across re-publications; searching by title is the robust approach.
"""
from __future__ import annotations

import geopandas as gpd
import pandas as pd
import requests

CKAN_BASE = "https://opendata-ajuntament.barcelona.cat/data/api/3/action"


def _package_search(query: str) -> list[dict]:
    resp = requests.get(
        f"{CKAN_BASE}/package_search", params={"q": query}, timeout=30
    )
    resp.raise_for_status()
    payload = resp.json()
    if not payload.get("success"):
        raise RuntimeError(f"CKAN search failed for query={query!r}: {payload}")
    return payload["result"]["results"]


def find_dataset_resources(query: str) -> pd.DataFrame:
    """Search Open Data BCN by free-text query, return a flat table of
    (dataset title, resource name, resource format, resource url) for the
    caller to inspect and pick from — CKAN doesn't guarantee a stable
    single "the" dataset per topic, so this hands back candidates rather
    than guessing which one is authoritative."""
    results = _package_search(query)
    rows = []
    for pkg in results:
        for res in pkg.get("resources", []):
            rows.append(
                {
                    "dataset_title": pkg.get("title"),
                    "dataset_name": pkg.get("name"),
                    "resource_name": res.get("name"),
                    "format": res.get("format"),
                    "url": res.get("url"),
                }
            )
    return pd.DataFrame(rows)


def load_geojson_resource(url: str) -> gpd.GeoDataFrame:
    """Fetch a specific CKAN resource URL that's already been confirmed
    (via find_dataset_resources) to be a GeoJSON export, and load it."""
    return gpd.read_file(url)
