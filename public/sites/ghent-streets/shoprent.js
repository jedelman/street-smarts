// What a Ghent ground floor might rent for as a shop: comparison, indexed to home values. Ghent's historic district
// has no shop market to observe (today the use is prohibited), so the estimate borrows from neighborhoods like it in
// five other cities. Each comparable's asking retail rent ($/sq ft/yr) is divided by its ZIP code's typical home value
// (Zillow's ZHVI, the same month for every ZIP), and that ratio is applied to Ghent's ZIP codes. The spread across the
// comparables is the range. Data: shoprents.json (tools/ghent/shoprents.py, from sources quoted there).
//
// Pure: tools/sparrow/shoprent.test.mjs tests it in Node.

/** Each comparable's rent per $100,000 of home value, and what that ratio gives in each of Ghent's ZIPs. */
export function indexed(data) {
  const ghent = data.ghent_zips.map((z) => ({ zip: z.zip, value: z.value, label: z.label }));
  const rows = data.comparables.map((c) => {
    const mid = (c.rent_low + c.rent_high) / 2;
    const ratio = mid / (c.home_value / 100000); // $/sq ft/yr per $100k of typical home value
    return { ...c, mid, ratio, ghent: ghent.map((g) => ({ zip: g.zip, rent: ratio * (g.value / 100000) })) };
  });
  return { rows, ghent };
}

/** The range across comparables in one of Ghent's ZIPs: low, median and high $/sq ft/yr. */
export function range(ix, zip) {
  const v = ix.rows.map((r) => r.ghent.find((g) => g.zip === zip).rent).sort((a, b) => a - b);
  const mid = v.length % 2 ? v[(v.length - 1) / 2] : (v[v.length / 2 - 1] + v[v.length / 2]) / 2;
  return { low: v[0], median: mid, high: v[v.length - 1], n: v.length };
}

/** A month's rent for `sqft` at `perSqftYear`. */
export const monthly = (perSqftYear, sqft) => (perSqftYear * sqft) / 12;
