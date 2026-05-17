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

// Determine ring orientation. ESRI: clockwise = outer, counter-clockwise = hole.
// In lng/lat where lat increases northward, signed area > 0 = clockwise.
function signedArea(ring) {
  let s = 0;
  for (let i = 0; i < ring.length; i++) {
    const [x1, y1] = ring[i];
    const [x2, y2] = ring[(i + 1) % ring.length];
    s += x1 * y2 - x2 * y1;
  }
  return s * 0.5;
}

// Test whether `inner`'s centroid lies inside `outer`. Crude (centroid only)
// but sufficient for separating disjoint parts from genuine holes in this data.
function ringCentroid(ring) {
  let lng = 0, lat = 0;
  for (const [x, y] of ring) { lng += x; lat += y; }
  return [lng / ring.length, lat / ring.length];
}

function pointInRing(pt, ring) {
  let inside = false;
  let j = ring.length - 1;
  for (let i = 0; i < ring.length; i++) {
    const [xi, yi] = ring[i];
    const [xj, yj] = ring[j];
    if (((yi > pt[1]) !== (yj > pt[1])) &&
        (pt[0] < (xj - xi) * (pt[1] - yi) / (yj - yi) + xi)) {
      inside = !inside;
    }
    j = i;
  }
  return inside;
}

/**
 * Group ESRI multi-rings into (outer, [holes]) parts.
 * - Rings with negative signed area whose centroid lies inside some other
 *   ring are treated as holes of that ring.
 * - All other rings become separate parts.
 *
 * For the EC data this correctly identifies MALL_CORE and parcel 00001129 as
 * two-part polygons rather than polygon-with-hole.
 */
function ringsToParts(rings) {
  const outers = [];
  const candidateHoles = [];
  for (const ring of rings) {
    if (signedArea(ring) > 0) {
      outers.push({ ring, holes: [] });
    } else {
      candidateHoles.push(ring);
    }
  }
  if (outers.length === 0) {
    for (const ring of rings) outers.push({ ring, holes: [] });
  } else {
    for (const hole of candidateHoles) {
      const c = ringCentroid(hole);
      const host = outers.find(o => pointInRing(c, o.ring));
      if (host) host.holes.push(hole);
      else outers.push({ ring: hole, holes: [] });
    }
  }
  // Sort parts by absolute area descending so `parts[0]` is always the largest.
  outers.sort((a, b) => Math.abs(signedArea(b.ring)) - Math.abs(signedArea(a.ring)));
  return outers.map(o => ({
    outer: ringToLngLat(o.ring),
    holes: o.holes.map(ringToLngLat),
  }));
}

function parcelsToNIR(parcels, label, idPrefix) {
  const allRings = parcels.flatMap(p => p.rings);
  const bbox = bboxOf(allRings);
  return {
    id: idPrefix,
    bbox_wgs84: bbox,
    parcels: parcels.map((p, i) => {
      const parts = ringsToParts(p.rings);
      const primary = parts[0] || { outer: [], holes: [] };
      return {
        id: p.acct || `${idPrefix}-parcel-${i}`,
        polygon: {
          outer: primary.outer,
          holes: primary.holes,
          parts: parts,
        },
        area_acres: p.area_ac || 0,
        use_category: null,
        ownership: null,
        is_eda: !!p.is_eda,
        spec: p.spec || null,
      };
    }),
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
