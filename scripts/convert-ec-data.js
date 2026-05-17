#!/usr/bin/env node
// Convert eastside-commons/ec-parcel-data.js → NIR JSON.
//
// The source file is a `const PARCEL_DATA = {...};` declaration; we evaluate
// it in a fresh VM context and re-shape into the street-smarts NIR schema.

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const args = process.argv.slice(2);
if (args.length < 2) {
  console.error('Usage: convert-ec-data.js <ec-parcel-data.js> <out-baseline.json> <out-proposal.json>');
  process.exit(1);
}
const [inputPath, outBaseline, outProposal] = args;

const src = fs.readFileSync(inputPath, 'utf8');
// Source uses `const PARCEL_DATA = {...};` which under `const` won't bind on
// the VM context. Rewrite the declaration prefix so the value lands on the
// context's `result` slot, then read from there.
const rewritten = src.replace(/^\s*const\s+PARCEL_DATA\s*=/m, 'result =');
const ctx = { result: null };
vm.createContext(ctx);
vm.runInContext(rewritten, ctx);
const data = ctx.result;
if (!data || !Array.isArray(data.parcels)) {
  console.error('Could not extract PARCEL_DATA.parcels');
  process.exit(2);
}

function bboxOf(rings) {
  let minLng = Infinity, minLat = Infinity, maxLng = -Infinity, maxLat = -Infinity;
  for (const ring of rings) {
    for (const [lng, lat] of ring) {
      if (lng < minLng) minLng = lng;
      if (lat < minLat) minLat = lat;
      if (lng > maxLng) maxLng = lng;
      if (lat > maxLat) maxLat = lat;
    }
  }
  return [minLng, minLat, maxLng, maxLat];
}

function ringToLngLat(ring) {
  return ring.map(([lng, lat]) => ({ lng, lat }));
}

function parcelsToNIR(parcels, label, idPrefix) {
  const allRings = parcels.flatMap(p => p.rings);
  const bbox = bboxOf(allRings);
  return {
    id: idPrefix,
    bbox_wgs84: bbox,
    parcels: parcels.map((p, i) => ({
      id: p.acct || `${idPrefix}-parcel-${i}`,
      polygon: {
        outer: ringToLngLat(p.rings[0]),
        holes: (p.rings.slice(1) || []).map(ringToLngLat),
      },
      area_acres: p.area_ac || 0,
      use_category: null,
      ownership: null, // baseline: ownership unknown; proposal: inferred via is_eda
      is_eda: !!p.is_eda,
      spec: p.spec || null,
    })),
    buildings: [],
    streets: [],
    open_space: [],
    boundaries: [],
    activity_nodes: [],
    metadata: {
      source: 'jedelman/jason-edelman.org/eastside-commons/ec-parcel-data.js',
      fetched_at: new Date().toISOString(),
      license: 'Norfolk City GIS (public records)',
      layer_provenance: {},
      label: label,
    },
  };
}

// Baseline: present-day Eastside / Military Circle. is_eda parcels do not yet
// exist on the ground; for the baseline view we exclude them and pretend the
// site is the pre-proposal fabric (the parking-lot wasteland).
const baseline = parcelsToNIR(
  data.parcels.filter(p => !p.is_eda),
  'Eastside Commons — current parcel fabric (Military Circle, Norfolk, May 2026)',
  'eastside-commons-baseline'
);

// Proposal: include all parcels, the EDA ones representing the Eastside Commons
// design. Ownership for EDA parcels is inferred from spec codes downstream.
const proposal = parcelsToNIR(
  data.parcels,
  'Eastside Commons — proposed scheme (EC_FieldSolver output)',
  'eastside-commons-proposal'
);

fs.writeFileSync(outBaseline, JSON.stringify(baseline));
fs.writeFileSync(outProposal, JSON.stringify(proposal));
console.error(`Wrote ${baseline.parcels.length} parcels → ${outBaseline}`);
console.error(`Wrote ${proposal.parcels.length} parcels → ${outProposal}`);
