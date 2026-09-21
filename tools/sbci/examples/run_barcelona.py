"""Barcelona run. Sants district (La Borda / Can Batlló area), chosen
because it's where the ESS seed fixture's coordinates cluster."""
import sys
from pathlib import Path

PACKAGE_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PACKAGE_ROOT))

from sbci.grid import UTM_BARCELONA  # noqa: E402
from sbci.pipeline import run_spatial_capital_analysis  # noqa: E402

# Sants district, Barcelona, roughly
BARCELONA_BBOX = (2.120, 41.368, 2.145, 41.383)

if __name__ == "__main__":
    result = run_spatial_capital_analysis(
        city_name="Barcelona",
        bbox_wgs84=BARCELONA_BBOX,
        projected_crs=UTM_BARCELONA,
        ess_points_path=str(PACKAGE_ROOT / "data" / "ess_nodes_barcelona_seed.geojson"),
        output_dir=str(PACKAGE_ROOT / "output"),
    )
    for w in result.warnings:
        print("WARNING:", w)
    print(result.grid[["scci_norm", "sced_norm", "sei_norm", "sbci"]].describe())
