"""Barcelona run. Sants district (La Borda / Can Batlló area), chosen
because it's where the ESS seed fixture's coordinates cluster."""
from sbci.grid import UTM_BARCELONA
from sbci.pipeline import run_spatial_capital_analysis

# Sants district, Barcelona, roughly
BARCELONA_BBOX = (2.120, 41.368, 2.145, 41.383)

if __name__ == "__main__":
    result = run_spatial_capital_analysis(
        city_name="Barcelona",
        bbox_wgs84=BARCELONA_BBOX,
        projected_crs=UTM_BARCELONA,
        ess_points_path="../data/ess_nodes_barcelona_seed.geojson",
        output_dir="../output",
    )
    for w in result.warnings:
        print("WARNING:", w)
    print(result.grid[["scci_norm", "sced_norm", "sei_norm", "sbci"]].describe())
