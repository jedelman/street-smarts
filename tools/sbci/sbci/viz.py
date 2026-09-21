"""Dual-panel SBCI heatmaps + folium isochrone overlay maps."""
from __future__ import annotations

from pathlib import Path

import folium
import geopandas as gpd
import matplotlib.pyplot as plt


def dual_panel_heatmap(
    norfolk_grid: gpd.GeoDataFrame,
    barcelona_grid: gpd.GeoDataFrame,
    column: str = "sbci",
    output_path: str | Path = "sbci_dual_panel.png",
) -> Path:
    output_path = Path(output_path)
    fig, axes = plt.subplots(1, 2, figsize=(16, 8))

    vmin = min(norfolk_grid[column].min(), barcelona_grid[column].min())
    vmax = max(norfolk_grid[column].max(), barcelona_grid[column].max())

    norfolk_grid.plot(
        column=column, cmap="viridis", legend=True, ax=axes[0], vmin=vmin, vmax=vmax
    )
    axes[0].set_title(f"Norfolk, VA — {column}")
    axes[0].set_axis_off()

    barcelona_grid.plot(
        column=column, cmap="viridis", legend=True, ax=axes[1], vmin=vmin, vmax=vmax
    )
    axes[1].set_title(f"Barcelona — {column}")
    axes[1].set_axis_off()

    fig.suptitle(f"Spatial Bio-Capital Index — {column}, shared color scale")
    fig.tight_layout()
    fig.savefig(output_path, dpi=200)
    plt.close(fig)
    return output_path


def isochrone_overlay_map(
    center_wgs84: tuple[float, float],
    isochrone_polygons_wgs84: gpd.GeoDataFrame,
    ess_nodes_wgs84: gpd.GeoDataFrame,
    barrier_features_wgs84: gpd.GeoDataFrame | None = None,
    output_path: str | Path = "isochrone_overlay.html",
) -> Path:
    """isochrone_polygons_wgs84 must be pre-computed by the caller — this
    module doesn't compute isochrones itself (that needs a routing engine
    like pandana/OSRM against the walk/transit graph, not implemented here).
    """
    output_path = Path(output_path)
    lat, lon = center_wgs84[1], center_wgs84[0]
    m = folium.Map(location=[lat, lon], zoom_start=15, tiles="cartodbpositron")

    folium.GeoJson(
        isochrone_polygons_wgs84.to_json(),
        name="15-min catchment",
        style_function=lambda _: {
            "fillColor": "#2c7fb8",
            "color": "#2c7fb8",
            "fillOpacity": 0.15,
            "weight": 1,
        },
    ).add_to(m)

    for _, row in ess_nodes_wgs84.iterrows():
        folium.CircleMarker(
            location=[row.geometry.y, row.geometry.x],
            radius=5 + 4 * row.get("weight", 0.5),
            color="#238b45",
            fill=True,
            fill_opacity=0.8,
            popup=row.get("name", "ESS node"),
        ).add_to(m)

    if barrier_features_wgs84 is not None and len(barrier_features_wgs84):
        folium.GeoJson(
            barrier_features_wgs84.to_json(),
            name="enclosures",
            style_function=lambda _: {"color": "#e34a33", "weight": 3},
        ).add_to(m)

    folium.LayerControl().add_to(m)
    m.save(str(output_path))
    return output_path
