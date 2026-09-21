"""run_spatial_capital_analysis() — orchestrates the full per-city pipeline.

This is the concrete implementation of the brief's Section 5 execution
outline. It calls real network/API code (osm_source, census_source,
opendata_bcn) — nothing here is mocked — so running it end-to-end depends
on those services being reachable from wherever you run it. See the
package README's live/stubbed table.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

import geopandas as gpd
import osmnx as ox

from . import osm_source
from .composite import compute_sbci
from .ess_source import load_ess_nodes
from .grid import build_grid
from .scci import compute_scci
from .sced import compute_sced
from .sei import compute_sei
from .survey import SurveyReport, build_survey, print_report


@dataclass
class CityRunResult:
    city_name: str
    grid: gpd.GeoDataFrame
    ess_nodes: gpd.GeoDataFrame
    survey: SurveyReport
    warnings: list[str] = field(default_factory=list)


def run_spatial_capital_analysis(
    city_name: str,
    bbox_wgs84: tuple[float, float, float, float],
    projected_crs: str,
    ess_points_path: str,
    output_dir: str | Path,
    cell_size_m: float = 100.0,
) -> CityRunResult:
    """
    1. Build the 100m grid over bbox
    2. Fetch OSM street network + building footprints for bbox
    3. Load ESS node layer from ess_points_path
    4. Compute SCCI, SCED, SEI per cell
    5. Composite SBCI, export GeoPackage + summary stats
    """
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    warnings: list[str] = []

    grid = build_grid(bbox_wgs84, projected_crs, cell_size_m)

    buildings = osm_source.fetch_building_footprints(bbox_wgs84)
    buildings = buildings.to_crs(projected_crs)

    drive_graph = osm_source.fetch_street_network(bbox_wgs84, network_type="drive")
    drive_edges = ox.graph_to_gdfs(drive_graph, nodes=False).to_crs(projected_crs)

    walk_graph = osm_source.fetch_street_network(bbox_wgs84, network_type="walk")
    walk_graph = ox.project_graph(walk_graph, to_crs=projected_crs)

    barrier_edges = drive_edges[drive_edges["highway"].apply(_is_barrier_highway)]

    ess_nodes = load_ess_nodes(ess_points_path)
    ess_nodes = ess_nodes.to_crs(projected_crs)
    if (~ess_nodes["verified"]).any():
        n_unverified = int((~ess_nodes["verified"]).sum())
        warnings.append(
            f"{n_unverified}/{len(ess_nodes)} ESS nodes are unverified "
            "(seed/approximate coordinates) — see data source docstring."
        )

    grid = compute_scci(grid, buildings, road_network_edges=drive_edges)
    grid = compute_sced(grid, ess_nodes)
    grid = compute_sei(grid, walk_graph, barrier_features=barrier_edges)
    grid = compute_sbci(grid)  # one optional lens, not the report — see survey.py

    grid.to_file(output_dir / f"sbci_grid_{city_name.lower()}.gpkg", driver="GPKG")
    ess_nodes.to_file(
        output_dir / "ess_nodes_spatial.geojson", driver="GeoJSON"
    )

    summary = grid[["scci_norm", "sced_norm", "sei_norm", "sbci"]].describe()
    summary.to_csv(output_dir / f"sbci_summary_{city_name.lower()}.csv")

    survey = build_survey(grid, city_name)
    print_report(survey)

    return CityRunResult(
        city_name=city_name,
        grid=grid,
        ess_nodes=ess_nodes,
        survey=survey,
        warnings=warnings,
    )


def _is_barrier_highway(highway_value) -> bool:
    from .sei import BARRIER_HIGHWAY_TYPES

    if isinstance(highway_value, list):
        return any(v in BARRIER_HIGHWAY_TYPES for v in highway_value)
    return highway_value in BARRIER_HIGHWAY_TYPES
