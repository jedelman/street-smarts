"""Norfolk, VA run. Eastside Commons / Military Circle area, echoing the
bbox street-smarts already uses for its own Norfolk fixtures (see
data/eastside-baseline.json), widened to a ~2km study area per the brief's
"dispersed industrial/military infrastructure" framing."""
from sbci.grid import UTM_NORFOLK
from sbci.pipeline import run_spatial_capital_analysis

# Eastside Commons / Military Circle, Norfolk VA, roughly
NORFOLK_BBOX = (-76.265, 36.865, -76.235, 36.885)

if __name__ == "__main__":
    result = run_spatial_capital_analysis(
        city_name="Norfolk",
        bbox_wgs84=NORFOLK_BBOX,
        projected_crs=UTM_NORFOLK,
        ess_points_path="../data/ess_nodes_norfolk_seed.geojson",
        output_dir="../output",
    )
    for w in result.warnings:
        print("WARNING:", w)
    print(result.grid[["scci_norm", "sced_norm", "sei_norm", "sbci"]].describe())
