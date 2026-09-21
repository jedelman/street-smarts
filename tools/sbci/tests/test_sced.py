import geopandas as gpd
from shapely.geometry import Point, box

from sbci.grid import UTM_NORFOLK
from sbci.sced import compute_sced


def _synthetic_grid():
    cells = [box(x, 0, x + 100, 100) for x in range(0, 500, 100)]
    return gpd.GeoDataFrame(
        {"cell_id": range(len(cells))}, geometry=cells, crs=UTM_NORFOLK
    )


def test_sced_decays_with_distance():
    grid = _synthetic_grid()
    # single node at the center of cell 0
    node = gpd.GeoDataFrame(
        {"weight": [1.0], "category": ["coop_housing_clt"]},
        geometry=[Point(50, 50)],
        crs=UTM_NORFOLK,
    )
    out = compute_sced(grid, node, bandwidth_m=1000.0)
    # sced should strictly decrease as cells get farther from the node
    values = out.sort_values("cell_id")["sced_raw"].tolist()
    assert values == sorted(values, reverse=True)
    assert values[0] > 0


def test_sced_empty_nodes_abstains_not_zero_measurement():
    grid = _synthetic_grid()
    empty_nodes = gpd.GeoDataFrame(
        {"weight": [], "category": []}, geometry=[], crs=UTM_NORFOLK
    )
    out = compute_sced(grid, empty_nodes)
    assert (out["sced_raw"] == 0).all()
    assert (out["sced_norm"] == 0).all()
    # the interesting assertion: an empty layer must be flagged as "no data",
    # not silently indistinguishable from "measured zero commons"
    assert (out["sced_abstain"] == True).all()  # noqa: E712
    assert out["sced_abstain_reason"].notna().all()


def test_sced_nonempty_nodes_does_not_abstain():
    grid = _synthetic_grid()
    node = gpd.GeoDataFrame(
        {"weight": [1.0], "category": ["coop_housing_clt"], "verified": [True]},
        geometry=[Point(50, 50)],
        crs=UTM_NORFOLK,
    )
    out = compute_sced(grid, node)
    assert (out["sced_abstain"] == False).all()  # noqa: E712


def test_sced_weight_scales_linearly():
    grid = _synthetic_grid()
    node_low = gpd.GeoDataFrame(
        {"weight": [0.5], "category": ["standard_commercial"]},
        geometry=[Point(50, 50)],
        crs=UTM_NORFOLK,
    )
    node_high = gpd.GeoDataFrame(
        {"weight": [1.0], "category": ["coop_housing_clt"]},
        geometry=[Point(50, 50)],
        crs=UTM_NORFOLK,
    )
    out_low = compute_sced(grid, node_low)
    out_high = compute_sced(grid, node_high)
    ratio = out_high["sced_raw"].iloc[0] / out_low["sced_raw"].iloc[0]
    assert abs(ratio - 2.0) < 1e-9
