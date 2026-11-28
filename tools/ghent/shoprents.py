"""
shoprents.py: what a Ghent ground floor might rent for as a shop, by comparison. Writes
public/sites/ghent-streets/shoprents.json for the Money panel (shoprent.js does the arithmetic in the walk).

Ghent's historic district has no shop market to observe: the use is prohibited today. So the estimate borrows from
neighborhoods like it in five other cities (Asheville, Charleston, Raleigh, Charlottesville and Savannah), as Jason
asked: each comparable's asking retail rent, divided by its ZIP code's typical home value, applied to Ghent's.

  - Rents: data/ghent/shop_rent_sources.json, one entry per comparable, each with its source, the URL it was read at,
    the sentence or table cell quoted word for word, the lease type and the date. Norfolk's own figures near Ghent
    are there too, marked "check": they're shown beside the estimate, not used in it.
  - Home values: Zillow's Home Value Index (ZHVI, all homes, middle tier, smoothed and seasonally adjusted) by ZIP
    code, the same month for every ZIP (Zip_zhvi_uc_sfrcondo_tier_0.33_0.67_sm_sa_month.csv, files.zillowstatic.com).

Run (network): tools/vibe-render/.venv/bin/python tools/ghent/shoprents.py
"""

import csv
import io
import json
import os
import sys
import urllib.request

sys.path.insert(0, os.path.dirname(__file__))
import survey  # noqa: E402

ROOT = survey.ROOT
SOURCES = os.path.join(ROOT, "data/ghent/shop_rent_sources.json")
OUT = os.path.join(ROOT, "public/sites/ghent-streets/shoprents.json")
ZHVI = "https://files.zillowstatic.com/research/public_csvs/zhvi/Zip_zhvi_uc_sfrcondo_tier_0.33_0.67_sm_sa_month.csv"
GHENT_ZIPS = [("23507", "West Ghent"), ("23517", "Ghent"), ("23510", "downtown's edge")]


def zhvi():
    key = os.path.join(survey.CACHE, "zillow_zip_zhvi.csv")
    if not os.path.exists(key):
        with urllib.request.urlopen(urllib.request.Request(ZHVI, headers=survey.UA), timeout=300) as r:
            data = r.read()
        os.makedirs(survey.CACHE, exist_ok=True)
        open(key, "wb").write(data)
    rows = list(csv.reader(io.open(key, encoding="utf-8")))
    head = rows[0]
    month = head[-1]
    values = {r[2]: (float(r[-1]) if r[-1] else None) for r in rows[1:]}
    return month, values


def main():
    src = json.load(open(SOURCES))
    month, values = zhvi()
    comps, checks = [], []
    for c in src["comparables"]:
        v = values.get(c["zip"])
        if not v:
            sys.exit(f"no ZHVI for ZIP {c['zip']} ({c['city']}, {c['district']})")
        row = {**c, "home_value": round(v), "home_value_month": month}
        (checks if c.get("check") else comps).append(row)
    # Each Ghent lot's ZIP, from the City's FY26 assessment (only the parcel's id and ZIP are requested).
    S = json.load(open(os.path.join(ROOT, "data/ghent/survey.json")))
    gpins = [p["gpin"] for p in S["parcels"]]
    lot_zips = {r["gpin"]: (r.get("property_zip") or "")[:5] for r in survey.socrata("m5ya-5grb", "gpin", gpins, "gpin,property_zip")
                if r.get("property_zip")}
    out = {
        "version": 1,
        "method": "Each comparable's asking retail rent ($/sq ft/yr, the midpoint of its range) divided by its ZIP code's "
                  "typical home value, applied to Ghent's ZIP codes. Lease types differ (triple net excludes taxes, "
                  "insurance and upkeep, which a gross rent includes); each row says which.",
        "home_values": f"Zillow Home Value Index (all homes, middle tier, smoothed, seasonally adjusted), by ZIP code, {month}",
        "rule": src["rule"],
        "ghent_zips": [{"zip": z, "label": label, "value": round(values[z]), "month": month} for z, label in GHENT_ZIPS],
        "comparables": comps,
        "checks": checks,
        "left_out": src.get("left_out", []),
        # gpin -> ZIP (the City's assessment), for each lot's own estimate
        "lot_zips": dict(sorted(lot_zips.items())),
    }
    with open(OUT, "w") as f:
        json.dump(out, f, indent=1, ensure_ascii=False)
        f.write("\n")
    print(f"shoprents.json: {len(comps)} comparables, {len(checks)} Norfolk checks; ZHVI {month}")


if __name__ == "__main__":
    main()
