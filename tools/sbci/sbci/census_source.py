"""US Census ACS 5-Year block-group pulls for Norfolk, VA.

Live endpoint (tested 302->200 with a key from this sandbox). Requires
CENSUS_API_KEY — the API returns an HTML "Missing Key" page without one,
which we detect and raise on rather than parsing as data.

Get a free key: https://api.census.gov/data/key_signup.html
"""
from __future__ import annotations

import os

import pandas as pd
import requests

ACS_YEAR = 2022
ACS_BASE = f"https://api.census.gov/data/{ACS_YEAR}/acs/acs5"

# Norfolk, VA: state FIPS 51, county FIPS 710 (independent city)
NORFOLK_STATE_FIPS = "51"
NORFOLK_COUNTY_FIPS = "710"

# Variables per the brief: median income, commute mode share, vehicle
# ownership per household.
VARIABLES = {
    "B19013_001E": "median_household_income",
    "B08301_010E": "commute_public_transit",
    "B08301_019E": "commute_walked",
    "B08301_001E": "commute_total",
    "B08201_002E": "households_zero_vehicles",
    "B08201_001E": "households_total",
}


def fetch_norfolk_block_groups() -> pd.DataFrame:
    """One row per Norfolk block group, columns per VARIABLES plus GEOID."""
    api_key = os.environ.get("CENSUS_API_KEY")
    if not api_key:
        raise RuntimeError(
            "CENSUS_API_KEY not set. Get one at "
            "https://api.census.gov/data/key_signup.html and export it."
        )

    get_vars = ",".join(["NAME", *VARIABLES.keys()])
    url = (
        f"{ACS_BASE}?get={get_vars}"
        f"&for=block%20group:*"
        f"&in=state:{NORFOLK_STATE_FIPS}%20county:{NORFOLK_COUNTY_FIPS}%20tract:*"
        f"&key={api_key}"
    )
    resp = requests.get(url, timeout=30)
    resp.raise_for_status()
    if resp.text.lstrip().startswith("<"):
        raise RuntimeError(
            "Census API returned HTML, not JSON — usually an invalid or "
            "missing key. Response body:\n" + resp.text[:500]
        )

    rows = resp.json()
    header, *data = rows
    df = pd.DataFrame(data, columns=header)
    df = df.rename(columns=VARIABLES)
    df["GEOID"] = df["state"] + df["county"] + df["tract"] + df["block group"]
    for col in VARIABLES.values():
        df[col] = pd.to_numeric(df[col], errors="coerce")

    df["vehicle_ownership_rate"] = 1 - (
        df["households_zero_vehicles"] / df["households_total"]
    )
    df["transit_walk_mode_share"] = (
        df["commute_public_transit"] + df["commute_walked"]
    ) / df["commute_total"]
    return df
