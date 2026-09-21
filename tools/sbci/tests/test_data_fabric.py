from sbci.data_fabric import SourceAssessment, assess_data_fabric


def test_default_sources_show_the_asymmetry():
    assessment = assess_data_fabric()
    assert assessment.capital_bulk_fraction > assessment.commons_bulk_fraction
    assert assessment.commons_bulk_fraction == 0.0


def test_every_commons_source_is_manual_directory_in_default_set():
    assessment = assess_data_fabric()
    commons = [s for s in assessment.sources if s.side == "commons"]
    assert len(commons) >= 2
    assert all(s.tier == "manual_directory" for s in commons)


def test_symmetric_synthetic_sources_show_no_gap():
    symmetric = [
        SourceAssessment("cap-a", "capital", "bulk_api", "test"),
        SourceAssessment("com-a", "commons", "bulk_api", "test"),
    ]
    assessment = assess_data_fabric(symmetric)
    assert assessment.capital_bulk_fraction == assessment.commons_bulk_fraction == 1.0


def test_headline_and_question_are_nonempty_strings():
    assessment = assess_data_fabric()
    assert len(assessment.headline) > 0
    assert len(assessment.question_for_humans) > 0


def test_empty_side_yields_nan_not_crash():
    import math

    only_capital = [SourceAssessment("cap-a", "capital", "bulk_api", "test")]
    assessment = assess_data_fabric(only_capital)
    assert math.isnan(assessment.commons_bulk_fraction)
