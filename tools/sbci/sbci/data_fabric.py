"""Finding, not fixture: the capital/commons data-legibility asymmetry.

Every source this package actually touched during development (see the
README's live/stubbed table) got classified once, here, by how it can be
queried — not by what it contains. That's the axis this module cares
about: not "is there data about cooperatives" (there plainly is — XES and
Pam a Pam both publish real directories) but "can code ask for all of it
at once, the way it can ask a government API for every parcel in a bbox."

The asymmetry: every capital-adjacent source in this brief is either a
bulk API or, worst case, a documented lookup-only service with a real
endpoint. Every commons/ESS source is a browse-only web directory with no
API found at all. That gap is structural, not a research-effort artifact
of this one session — it means the tooling of the state and the market
got built assuming programmatic bulk access matters; the tooling of the
solidarity economy didn't. A pipeline like this one will always be able
to say more, with less effort, about capital than about commons, for
reasons that have nothing to do with which one is actually bigger on the
ground. That's worth naming before anyone reads a low SCED number as "not
much commons economy here" instead of "not much of the commons economy
chose to expose itself to a script."

n is small (six sources, one session, two cities) — this is an anecdote
with receipts, not a stat. Don't compute a percentage and repeat it like
it generalizes past what was actually tested.
"""
from __future__ import annotations

from dataclasses import dataclass

LEGIBILITY_TIERS = [
    "bulk_api",  # bbox/region query, no per-record lookup required
    "bulk_api_keyed",  # bulk_api but gated behind a registration/API key
    "lookup_only",  # real API, but one record (parcel/address) at a time
    "manual_directory",  # browse-only web directory, no API found
]


@dataclass
class SourceAssessment:
    name: str
    side: str  # "capital" or "commons"
    tier: str  # one of LEGIBILITY_TIERS
    evidence: str  # what was actually tested, this session, not a claim


@dataclass
class FabricAssessment:
    sources: list[SourceAssessment]
    headline: str
    capital_bulk_fraction: float
    commons_bulk_fraction: float
    question_for_humans: str


# One row per source actually exercised while building this pipeline.
# Update this list, don't hardcode its conclusions elsewhere — README's
# fabric section and print_fabric_report() both read from here so the
# claim and the evidence can't drift apart.
SOURCES: list[SourceAssessment] = [
    SourceAssessment(
        name="OSM Overpass (street network, building footprints)",
        side="capital",
        tier="bulk_api",
        evidence="osmnx bbox queries against overpass-api.de; blocked from this "
        "particular sandbox's egress, but a real bbox-query API, no per-record "
        "lookup required.",
    ),
    SourceAssessment(
        name="US Census ACS (income, commute mode, vehicle ownership)",
        side="capital",
        tier="bulk_api_keyed",
        evidence="api.census.gov returns full block-group tables for a county "
        "in one call; gated behind a free API key (tested: 'Missing Key' "
        "without one).",
    ),
    SourceAssessment(
        name="Open Data BCN (municipal facilities, superblocks)",
        side="capital",
        tier="bulk_api",
        evidence="CKAN package_search, tested live: 122 hits for 'urbanisme', "
        "114 for 'transport', no key needed.",
    ),
    SourceAssessment(
        name="Catastro (parcel FAR / building age)",
        side="capital",
        tier="lookup_only",
        evidence="SOAP service keyed to one parcel reference or point at a "
        "time; no bulk bbox query found in the free tier. Still a documented, "
        "real, queryable endpoint — just not bulk.",
    ),
    SourceAssessment(
        name="XES (Xarxa d'Economia Solidària) directory",
        side="commons",
        tier="manual_directory",
        evidence="Browse-only web directory. No API endpoint found during this "
        "session.",
    ),
    SourceAssessment(
        name="Pam a Pam ESS map",
        side="commons",
        tier="manual_directory",
        evidence="Browse-only web directory/map. No bulk export or API endpoint "
        "found during this session.",
    ),
]


def _bulk_fraction(sources: list[SourceAssessment], side: str) -> float:
    side_sources = [s for s in sources if s.side == side]
    if not side_sources:
        return float("nan")
    bulk = sum(1 for s in side_sources if s.tier.startswith("bulk_api"))
    return bulk / len(side_sources)


def assess_data_fabric(sources: list[SourceAssessment] | None = None) -> FabricAssessment:
    sources = sources if sources is not None else SOURCES
    capital_frac = _bulk_fraction(sources, "capital")
    commons_frac = _bulk_fraction(sources, "commons")

    n_capital = sum(1 for s in sources if s.side == "capital")
    n_commons = sum(1 for s in sources if s.side == "commons")

    headline = (
        f"Of {n_capital} capital-adjacent sources tested, {capital_frac:.0%} "
        f"support bulk/bbox query. Of {n_commons} commons/ESS sources tested, "
        f"{commons_frac:.0%} do. Small n — treat as a documented pattern from "
        "this one pipeline-building session, not a general statistic."
    )

    return FabricAssessment(
        sources=sources,
        headline=headline,
        capital_bulk_fraction=capital_frac,
        commons_bulk_fraction=commons_frac,
        question_for_humans=(
            "This pipeline can say more, with less effort, about fixed capital "
            "than about the solidarity economy — not because there's less "
            "commons activity on the ground, but because commons-side "
            "infrastructure wasn't built for programmatic bulk access. Before "
            "reading any city's SCED as low, ask whether that's the ground "
            "truth or just what a script could reach."
        ),
    )


def print_fabric_report(assessment: FabricAssessment) -> None:
    print("\n=== Data fabric legibility: capital vs. commons ===")
    for s in assessment.sources:
        print(f"  [{s.side:>7}] {s.tier:<16} {s.name}")
        print(f"            {s.evidence}")
    print(f"\n{assessment.headline}")
    print(f"\n-- Question for humans --\n{assessment.question_for_humans}")
