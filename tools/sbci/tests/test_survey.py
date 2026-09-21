import geopandas as gpd
from shapely.geometry import box

from sbci.survey import build_survey


def _grid_row(scci, sced, sei, sced_abstain=False):
    return gpd.GeoDataFrame(
        {
            "cell_id": [0],
            "scci_norm": [scci],
            "sced_norm": [sced],
            "sei_norm": [sei],
            "sced_abstain": [sced_abstain],
            "sced_abstain_reason": ["no data" if sced_abstain else None],
        },
        geometry=[box(0, 0, 100, 100)],
        crs="EPSG:32618",
    )


def test_high_capital_high_commons_raises_a_question():
    grid = _grid_row(scci=0.8, sced=0.8, sei=0.5)
    report = build_survey(grid, "TestCity")
    assert len(report.questions_for_humans) == 1
    assert "coexist" in report.questions_for_humans[0].question


def test_abstained_cell_with_high_capital_flags_data_gap_not_capital_win():
    grid = _grid_row(scci=0.9, sced=0.0, sei=0.5, sced_abstain=True)
    report = build_survey(grid, "TestCity")
    assert len(report.questions_for_humans) == 1
    assert "No commons/ESS data" in report.questions_for_humans[0].question
    assert len(report.abstentions) == 1
    assert report.abstentions[0].cell_count == 1


def test_quiet_agreeing_cell_raises_no_questions():
    grid = _grid_row(scci=0.5, sced=0.5, sei=0.5)
    report = build_survey(grid, "TestCity")
    assert report.questions_for_humans == []


def test_all_low_flags_possible_data_gap():
    grid = _grid_row(scci=0.05, sced=0.05, sei=0.05)
    report = build_survey(grid, "TestCity")
    assert len(report.questions_for_humans) == 1
    assert "data-coverage gap" in report.questions_for_humans[0].question


def test_chorus_summary_present_for_all_three_metrics():
    grid = _grid_row(scci=0.5, sced=0.5, sei=0.5)
    report = build_survey(grid, "TestCity")
    labels = {c.metric for c in report.chorus}
    assert len(report.chorus) == 3
    assert any("SCCI" in l for l in labels)
    assert any("SCED" in l for l in labels)
    assert any("SEI" in l for l in labels)


def test_missing_columns_raises():
    import pytest

    bad_grid = gpd.GeoDataFrame(
        {"cell_id": [0]}, geometry=[box(0, 0, 100, 100)], crs="EPSG:32618"
    )
    with pytest.raises(ValueError):
        build_survey(bad_grid, "TestCity")
