"""The actual primary output: a survey instrument, not a verdict.

Mirrors street-smarts-conflict's DisagreementReport model directly — same
premise, applied to continuous spatial metrics instead of discrete
Alexander-pattern opinions: disagreement between SCCI/SCED/SEI is the
product, not noise to be averaged away. compute_sbci() in composite.py
still exists as one optional lens (a single number is sometimes useful
for a quick scan), but it is not what this module hands back, and nothing
downstream should treat it as "the score."

Three things this module produces per grid, deliberately not one:

  1. `questions_for_humans` — cells where the sub-metrics land somewhere
     specific enough to be worth a person actually looking, phrased as a
     question rather than a claim. High-capital-and-high-commons together
     is not "good" or "bad"; it's a place to go check.
  2. `abstentions` — cells (or whole layers) where a metric didn't
     measure, distinguished from a metric that measured and found little.
  3. `chorus_summary` — the plain aggregate spread per metric, so a reader
     sees how much the three metrics actually agree with each other before
     any per-cell story gets told.

Thresholds below are legible opinions, not calibrated statistics — same
posture as the brief's own governance-tier weights (co-op housing = 1.0,
standard commercial = 0.0). Change them in the open; don't treat 0.6 as
if it were derived from anything.
"""
from __future__ import annotations

from dataclasses import dataclass, field

import geopandas as gpd
import pandas as pd

# Legible, adjustable — not derived from data. See module docstring.
HIGH_THRESHOLD = 0.6
LOW_THRESHOLD = 0.2


@dataclass
class HumanPrompt:
    cell_id: int
    question: str
    metrics: dict[str, float]


@dataclass
class Abstention:
    metric: str
    cell_count: int
    reason: str


@dataclass
class ChorusSummary:
    metric: str
    mean: float
    std: float
    headline: str


@dataclass
class SurveyReport:
    city_name: str
    n_cells: int
    chorus: list[ChorusSummary]
    abstentions: list[Abstention]
    questions_for_humans: list[HumanPrompt]


def _chorus_summary(grid: gpd.GeoDataFrame, column: str, label: str) -> ChorusSummary:
    values = grid[column]
    mean, std = float(values.mean()), float(values.std())
    if std < 0.1:
        spread = "tightly clustered"
    elif std < 0.25:
        spread = "moderately spread"
    else:
        spread = "widely spread"
    headline = f"{label}: mean {mean:.2f}, {spread} (std {std:.2f}) across {len(grid)} cells"
    return ChorusSummary(metric=label, mean=mean, std=std, headline=headline)


def _row_questions(row: pd.Series) -> list[str]:
    questions = []
    scci, sced, sei = row["scci_norm"], row["sced_norm"], row["sei_norm"]
    sced_abstain = bool(row.get("sced_abstain", False))

    if sced_abstain and scci >= HIGH_THRESHOLD:
        return [
            "No commons/ESS data was collected for this cell, but it has "
            "substantial fixed-capital density. Before concluding anything "
            "about capital crowding out commons here — go check by hand."
        ]

    if not sced_abstain and scci >= HIGH_THRESHOLD and sced >= HIGH_THRESHOLD:
        questions.append(
            "Dense fixed capital AND dense commons/ESS activity in the same "
            "cell — does the commons here coexist with capital, get "
            "subsidized by proximity to it, or is it under pressure from it? "
            "The metric can't tell you which; go look."
        )

    if not sced_abstain and sei <= LOW_THRESHOLD and sced >= HIGH_THRESHOLD:
        questions.append(
            "Commons/ESS activity is dense here but the street network reads "
            "as enclosed/low-porosity — is this commons hard to physically "
            "reach from its surroundings? Worth an isochrone check."
        )

    if not sced_abstain and max(scci, sced, sei) <= LOW_THRESHOLD:
        questions.append(
            "All three metrics read low. Could be a genuinely empty/void "
            "space, or could be a data-coverage gap in this cell (patchy OSM "
            "tagging, missed ESS node) — distinguish before treating this as "
            "'nothing here'."
        )

    return questions


def build_survey(grid: gpd.GeoDataFrame, city_name: str) -> SurveyReport:
    required = {"scci_norm", "sced_norm", "sei_norm"}
    missing = required - set(grid.columns)
    if missing:
        raise ValueError(f"grid missing columns {missing}")

    chorus = [
        _chorus_summary(grid, "scci_norm", "SCCI (fixed-capital intensity)"),
        _chorus_summary(grid, "sced_norm", "SCED (commons/ESS density)"),
        _chorus_summary(grid, "sei_norm", "SEI (network porosity − enclosure)"),
    ]

    abstentions = []
    if "sced_abstain" in grid.columns:
        n_abstain = int(grid["sced_abstain"].sum())
        if n_abstain > 0:
            reason = grid.loc[grid["sced_abstain"], "sced_abstain_reason"].iloc[0]
            abstentions.append(
                Abstention(metric="sced", cell_count=n_abstain, reason=reason)
            )

    prompts: list[HumanPrompt] = []
    for _, row in grid.iterrows():
        for q in _row_questions(row):
            prompts.append(
                HumanPrompt(
                    cell_id=int(row["cell_id"]),
                    question=q,
                    metrics={
                        "scci_norm": float(row["scci_norm"]),
                        "sced_norm": float(row["sced_norm"]),
                        "sei_norm": float(row["sei_norm"]),
                    },
                )
            )

    return SurveyReport(
        city_name=city_name,
        n_cells=len(grid),
        chorus=chorus,
        abstentions=abstentions,
        questions_for_humans=prompts,
    )


def print_report(report: SurveyReport) -> None:
    print(f"\n=== Survey: {report.city_name} ({report.n_cells} cells) ===")
    print("\n-- Chorus --")
    for c in report.chorus:
        print(f"  {c.headline}")
    if report.abstentions:
        print("\n-- Abstentions --")
        for a in report.abstentions:
            print(f"  {a.metric}: {a.cell_count} cells — {a.reason}")
    print(f"\n-- Questions for humans ({len(report.questions_for_humans)}) --")
    for p in report.questions_for_humans[:20]:
        print(f"  [cell {p.cell_id}] {p.question}")
    if len(report.questions_for_humans) > 20:
        print(f"  ... and {len(report.questions_for_humans) - 20} more")
