"""Norfolk, VA run. Eastside Commons / Military Circle area, echoing the
bbox street-smarts already uses for its own Norfolk fixtures (see
data/eastside-baseline.json), widened to a ~2km study area per the brief's
"dispersed industrial/military infrastructure" framing."""
import sys
from pathlib import Path

PACKAGE_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PACKAGE_ROOT))

from sbci.grid import UTM_NORFOLK  # noqa: E402
from sbci.pipeline import run_spatial_capital_analysis  # noqa: E402

# Eastside Commons / Military Circle, Norfolk VA, roughly
NORFOLK_BBOX = (-76.265, 36.865, -76.235, 36.885)

if __name__ == "__main__":
    result = run_spatial_capital_analysis(
        city_name="Norfolk",
        bbox_wgs84=NORFOLK_BBOX,
        projected_crs=UTM_NORFOLK,
        ess_points_path=str(PACKAGE_ROOT / "data" / "ess_nodes_norfolk_seed.geojson"),
        output_dir=str(PACKAGE_ROOT / "output"),
    )
    for w in result.warnings:
        print("WARNING:", w)
    print(result.grid[["scci_norm", "sced_norm", "sei_norm", "sbci"]].describe())
