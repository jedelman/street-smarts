"""100m x 100m vector grid construction over a bounding box.

Pure geometry — no network calls, no external data. This is the spatial
primitive every other module in the pipeline joins against.
"""
from __future__ import annotations

import geopandas as gpd
import numpy as np
from shapely.geometry import box

# Meters-based projections per study area, chosen for low distortion at the
# city scale rather than a one-size-fits-all global projection.
UTM_NORFOLK = "EPSG:32618"  # UTM zone 18N
UTM_BARCELONA = "EPSG:32631"  # UTM zone 31N


def build_grid(
    bbox_wgs84: tuple[float, float, float, float],
    projected_crs: str,
    cell_size_m: float = 100.0,
) -> gpd.GeoDataFrame:
    """Build a regular square grid over a WGS84 bbox, in a projected CRS.

    Parameters
    ----------
    bbox_wgs84 : (min_lon, min_lat, max_lon, max_lat)
    projected_crs : an EPSG code (e.g. UTM_NORFOLK) with meter units
    cell_size_m : grid cell edge length in meters

    Returns
    -------
    GeoDataFrame with columns [cell_id, geometry], indexed 0..n-1, in
    `projected_crs`. Cells are clipped to the projected bbox extent — no
    partial-cell trimming against a real coastline/boundary, since callers
    supply the study-area bbox already.
    """
    min_lon, min_lat, max_lon, max_lat = bbox_wgs84
    if min_lon >= max_lon or min_lat >= max_lat:
        raise ValueError(f"degenerate bbox: {bbox_wgs84}")

    bbox_gdf = gpd.GeoDataFrame(
        geometry=[box(min_lon, min_lat, max_lon, max_lat)], crs="EPSG:4326"
    ).to_crs(projected_crs)
    minx, miny, maxx, maxy = bbox_gdf.total_bounds

    xs = np.arange(minx, maxx, cell_size_m)
    ys = np.arange(miny, maxy, cell_size_m)

    cells = [
        box(x, y, x + cell_size_m, y + cell_size_m) for x in xs for y in ys
    ]
    grid = gpd.GeoDataFrame(
        {"cell_id": range(len(cells))}, geometry=cells, crs=projected_crs
    )
    grid["cell_area_m2"] = grid.geometry.area
    return grid
