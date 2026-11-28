// Ghent's shop rents by comparison (shoprent.js): the index arithmetic on a toy set, and the built data's integrity.
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { indexed, range, monthly } from "../../public/sites/ghent-streets/shoprent.js";

test("a comparable's rent, as a share of its home values, applied to Ghent's", () => {
  const data = {
    ghent_zips: [{ zip: "23517", value: 300000 }, { zip: "23507", value: 450000 }],
    comparables: [
      { city: "A", rent_low: 20, rent_high: 30, home_value: 500000 }, // $25 at $500k: $5 per $100k
      { city: "B", rent_low: 40, rent_high: 40, home_value: 1000000 }, // $4 per $100k
      { city: "C", rent_low: 18, rent_high: 18, home_value: 300000 }, // $6 per $100k
    ],
  };
  const ix = indexed(data);
  assert.equal(ix.rows[0].ratio, 5);
  assert.equal(ix.rows[0].ghent[0].rent, 15); // $5 per $100k at $300k
  const r = range(ix, "23517");
  assert.deepEqual([r.low, r.median, r.high, r.n], [12, 15, 18, 3]);
  assert.equal(monthly(24, 1500), 3000);
});

const path = new URL("../../public/sites/ghent-streets/shoprents.json", import.meta.url);
test("Ghent's shop-rent comparables: sourced, quoted, and indexed to one month's home values", { skip: !existsSync(path) }, () => {
  const d = JSON.parse(readFileSync(path, "utf8"));
  assert.ok(d.comparables.length >= 4, "enough comparables to make a range");
  const months = new Set([...d.comparables.map((c) => c.home_value_month), ...d.ghent_zips.map((z) => z.month)]);
  assert.equal(months.size, 1, "every home value is the same month's");
  for (const c of d.comparables) {
    assert.ok(c.url && c.quote && c.source, `${c.city} ${c.district}: its source and quotation`);
    assert.ok(c.rent_low > 0 && c.rent_high >= c.rent_low && c.home_value > 50000, `${c.city} ${c.district}: plausible numbers`);
  }
});
