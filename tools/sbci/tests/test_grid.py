from sbci.grid import build_grid, UTM_NORFOLK


def test_build_grid_cell_count_and_area():
    # ~1km x 1km bbox near Norfolk, VA
    bbox = (-76.30, 36.84, -76.29, 36.85)
    grid = build_grid(bbox, UTM_NORFOLK, cell_size_m=100.0)

    assert len(grid) > 0
    assert set(grid.columns) >= {"cell_id", "geometry", "cell_area_m2"}
    # every cell should be ~100x100 = 10000 m^2 (allow float slack)
    assert all(abs(a - 10000.0) < 1.0 for a in grid["cell_area_m2"])


def test_build_grid_rejects_degenerate_bbox():
    import pytest

    with pytest.raises(ValueError):
        build_grid((-76.29, 36.85, -76.30, 36.84), UTM_NORFOLK)
