// The shapes of Ghent's streetlights, by fixture family (lamps.js draws them; tools/sparrow/lamps.test.mjs tests this
// in Node). The families are our reading of Dominion's fixture codes against its catalogue's styles; the shapes are ours.

const POLE_COLORS = { fiberglass: "#1d1d1d", concrete: "#9c968b", aluminum: "#8d9095", steel: "#6d6f72", wood: "#5a4632", other: "#2a2a2a" };
const WARM = "#ffd79a", COOL = "#fff1d6";
const FT = 0.3048;

/** Where each part of a light goes, in the light's own frame (y up, +z toward its street): [kind, x, y, z, sx, sy, sz, color]. */
export function lampParts(family, pole) {
  const P = POLE_COLORS[pole] || POLE_COLORS.other, out = [];
  const add = (k, x, y, z, sx, sy, sz, c) => out.push([k, x, y, z, sx, sy, sz, c]);
  if (family === "colonial" || family === "acorn") {
    const top = 13 * FT; // the catalogue's 13-foot pole
    const dark = pole === "concrete" ? "#2a2a2a" : P;
    add("pole", 0, 0.3, 0, 0.3, 0.6, 0.3, P); // the base
    add("pole", 0, top / 2 + 0.3, 0, 0.13, top - 0.6, 0.13, P);
    if (family === "colonial") {
      add("box", 0, top + 0.03, 0, 0.36, 0.06, 0.36, dark);
      add("glow", 0, top + 0.3, 0, 0.3, 0.48, 0.3, WARM);
      add("cone", 0, top + 0.66, 0, 0.5, 0.24, 0.5, dark);
      add("box", 0, top + 0.83, 0, 0.05, 0.1, 0.05, dark);
    } else {
      add("glowball", 0, top + 0.3, 0, 0.38, 0.56, 0.38, WARM);
      add("cone", 0, top + 0.64, 0, 0.22, 0.16, 0.22, dark);
    }
    add("pool", 0, 0.17, 0, 7, 1, 7, WARM);
    return out;
  }
  // On an arm, over the street: a cobra head, a shoebox, or (anything else) a plain head on a utility pole.
  const h = family === "shoebox" ? 7.6 : 8.8, arm = family === "shoebox" ? 1.1 : 1.9;
  add("pole", 0, h / 2, 0, family === "shoebox" ? 0.16 : 0.26, h, family === "shoebox" ? 0.16 : 0.26, P);
  add("box", 0, h - 0.25, arm / 2, 0.08, 0.08, arm, P === POLE_COLORS.wood ? "#7c7f82" : P);
  if (family === "cobra") {
    add("box", 0, h - 0.3, arm + 0.25, 0.34, 0.14, 0.7, "#8e9196");
    add("glow", 0, h - 0.39, arm + 0.25, 0.28, 0.03, 0.56, COOL);
  } else if (family === "shoebox") {
    add("box", 0, h - 0.3, arm + 0.2, 0.55, 0.16, 0.42, "#3a3c3f");
    add("glow", 0, h - 0.39, arm + 0.2, 0.5, 0.03, 0.38, COOL);
  } else {
    add("box", 0, h - 0.3, arm + 0.15, 0.3, 0.22, 0.36, "#5d6064");
    add("glow", 0, h - 0.43, arm + 0.15, 0.26, 0.04, 0.3, COOL);
  }
  add("pool", 0, 0.16, arm + 0.2, 12, 1, 12, COOL);
  return out;
}
