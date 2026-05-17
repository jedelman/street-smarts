// End-to-end: load WASM, parse fixture, generate report, exercise voice logic
import { readFileSync } from "node:fs";
import init, { analyze_neighborhood, version, list_operators, subdivide_parcel } from "../public/pkg/street_smarts_web.js";

const wasm = readFileSync("./public/pkg/street_smarts_web_bg.wasm");
await init({ module_or_path: wasm });

console.log("version:", version());
console.log("operators:", JSON.parse(list_operators()).map(o => o.name).join(", "));

const raw = readFileSync("./public/data/eastside-proposal.json", "utf8");
const neighborhood = JSON.parse(raw);
const reportJson = analyze_neighborhood(raw);
const report = JSON.parse(reportJson);

console.log("parcels:", neighborhood.parcels.length);
console.log("opinions on proposal:", report.opinions.map(o => `${o.opinion.name}=${o.output.kind === "value" ? o.output.value.toFixed(2) : "no_view"}`).join("  "));

// Subdivide MALL_CORE across a few seeds, re-run the chorus on each.
console.log("\n=== P95 SUBDIVISION OF MALL_CORE ===");
for (const seed of [1n, 7n, 42n]) {
  const result = subdivide_parcel(raw, "13279568", "p95_building_complex", seed);
  const parsed = JSON.parse(result);
  console.log(`\nseed=${seed}: ${parsed.trace.headline}`);
  console.log(`  parcels: ${neighborhood.parcels.length} -> ${parsed.neighborhood.parcels.length}; open_space: ${parsed.neighborhood.open_space.length}`);
  const modReport = JSON.parse(analyze_neighborhood(JSON.stringify(parsed.neighborhood)));
  console.log(`  chorus on subdivided: ${modReport.opinions.map(o => `${o.opinion.name}=${o.output.kind === "value" ? o.output.value.toFixed(2) : "no_view"}`).join("  ")}`);
}

