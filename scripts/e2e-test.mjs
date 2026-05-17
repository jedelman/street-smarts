// End-to-end: load WASM, parse fixture, generate report, exercise voice logic
import { readFileSync } from "node:fs";
import init, { analyze_neighborhood, version } from "../public/pkg/street_smarts_web.js";

const wasm = readFileSync("./public/pkg/street_smarts_web_bg.wasm");
await init({ module_or_path: wasm });

console.log("version:", version());

const raw = readFileSync("./public/data/eastside-proposal.json", "utf8");
const neighborhood = JSON.parse(raw);
const reportJson = analyze_neighborhood(raw);
const report = JSON.parse(reportJson);

console.log("bbox:", neighborhood.bbox_wgs84);
console.log("parcels:", neighborhood.parcels.length);
console.log("opinions:", report.opinions.map(o => `${o.opinion.name}=${o.output.kind === "value" ? o.output.value.toFixed(2) : "no_view"}`).join("  "));

// Simulate the strong_centers voice's recoloring
const sc = report.opinions.find(o => o.opinion.name === "strong_centers");
if (sc?.output?.kind === "value") {
  console.log("strong_centers top contributing:", sc.output.contributing_features);
  console.log("strong_centers sub_scores:", sc.output.sub_scores);
  // What spec codes are those?
  for (const fid of sc.output.contributing_features) {
    const p = neighborhood.parcels.find(x => x.id === fid);
    console.log(`  ${fid}: spec=${p?.spec}, area=${p?.area_acres} ac, is_eda=${p?.is_eda}`);
  }
}

// Simulate ownership voice's bucket counts
const bucketOf = (p) => {
  if (!p.is_eda) return p.ownership === "private" ? "private" : "unknown";
  const s = p.spec || "";
  if (s.startsWith("CLT_")) return "clt";
  return "public";
};
const counts = {};
for (const p of neighborhood.parcels) {
  const b = bucketOf(p);
  counts[b] = (counts[b] || 0) + 1;
}
console.log("ownership buckets:", counts);
