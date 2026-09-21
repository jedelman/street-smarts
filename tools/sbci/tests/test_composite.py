import geopandas as gpd
import pytest
from shapely.geometry import box

from sbci.composite import compute_sbci


def _grid_with(scci, sced, sei):
    return gpd.GeoDataFrame(
        {
            "cell_id": [0],
            "scci_norm": [scci],
            "sced_norm": [sced],
            "sei_norm": [sei],
        },
        geometry=[box(0, 0, 100, 100)],
        crs="EPSG:32618",
    )


def test_sbci_high_commons_low_capital_scores_high():
    high_commons = compute_sbci(_grid_with(scci=0.0, sced=1.0, sei=1.0))
    high_capital = compute_sbci(_grid_with(scci=1.0, sced=0.0, sei=0.0))
    assert high_commons["sbci"].iloc[0] > high_capital["sbci"].iloc[0]


def test_sbci_missing_columns_raises():
    grid = gpd.GeoDataFrame(
        {"cell_id": [0]}, geometry=[box(0, 0, 100, 100)], crs="EPSG:32618"
    )
    with pytest.raises(ValueError):
        compute_sbci(grid)
