// What a shop front's sign says, and in what letters. Every front gets a name of its own, grown from a small
// grammar per kind of business ("The Copper Kettle", "Hartwell & Okafor, CPAs", "Side B Records"), and a face
// from those that suit its kind (a script for a café, a didone for a salon, a typewriter for a record shop).
// Ours, not data: no name here belongs to a real business, and the word banks leave out Norfolk's own street and
// place names, so a sign can't pass for a shop that's really there. Seeded by the front's id, so a front keeps its
// name between visits.
//
// The faces are eight open-licensed fonts served with the walk (public/sparrow/fonts, each with its licence).
//
// Pure, apart from loadSignFaces(): tools/sparrow/signs.test.mjs tests it in Node.

export const FACES = [
  { id: "playfair", family: "Playfair Display", weight: 700, style: "italic", file: "playfair-display-700i", caps: false },
  { id: "abril", family: "Abril Fatface", weight: 400, style: "normal", file: "abril-fatface", caps: true },
  { id: "bebas", family: "Bebas Neue", weight: 400, style: "normal", file: "bebas-neue", caps: true },
  { id: "pacifico", family: "Pacifico", weight: 400, style: "normal", file: "pacifico", caps: false },
  { id: "elite", family: "Special Elite", weight: 400, style: "normal", file: "special-elite", caps: true },
  { id: "josefin", family: "Josefin Sans", weight: 600, style: "normal", file: "josefin-sans-600", caps: true },
  { id: "archivo", family: "Archivo Black", weight: 400, style: "normal", file: "archivo-black", caps: true },
  { id: "cormorant", family: "Cormorant Garamond", weight: 700, style: "normal", file: "cormorant-garamond-700", caps: false },
];
const FACE = Object.fromEntries(FACES.map((f) => [f.id, f]));

// Which faces suit which kind of business; a business can narrow it.
const BY_FAMILY = {
  food: ["playfair", "pacifico", "abril", "cormorant", "josefin"],
  shop: ["cormorant", "abril", "archivo", "bebas", "josefin", "playfair"],
  office: ["cormorant", "josefin", "bebas"],
  studio: ["josefin", "elite", "bebas", "archivo"],
  school: ["archivo", "pacifico", "josefin"],
  salon: ["pacifico", "playfair", "abril", "josefin"],
  gallery: ["cormorant", "josefin", "playfair"],
  fitness: ["josefin", "bebas", "pacifico"],
};
const BY_BUSINESS = { records: ["elite", "bebas", "archivo"], books: ["cormorant", "playfair", "elite"], hardware: ["archivo", "bebas", "abril"],
  tailor: ["cormorant", "playfair", "abril"], barber: ["abril", "playfair", "bebas"], cleaners: ["josefin", "archivo", "bebas"] };

// Word banks: ours, ordinary, and free of Norfolk's own names.
const W = {
  surname: ["Alder", "Bellamy", "Castell", "Dunmore", "Ellery", "Farrow", "Galloway", "Hartwell", "Ibarra", "Jansen", "Kowal",
    "Lindqvist", "Marchetti", "Nakamura", "Okafor", "Pell", "Quill", "Rosales", "Sato", "Tremaine", "Underhill", "Varga",
    "Whitlock", "Yoon", "Zeller", "Abernathy", "Brandt", "Costa", "Delgado", "Esposito", "Fennimore", "Greer", "Holloway",
    "Iwata", "Kaplan", "Lowell", "Mendes", "Novak", "Osei", "Pruitt", "Ramos", "Sorensen", "Tanaka", "Vance", "Wren"],
  given: ["Ada", "Bea", "Celia", "Dom", "Etta", "Flo", "Gus", "Hal", "Ines", "June", "Kit", "Lou", "Mae", "Nell", "Otto",
    "Pia", "Rosa", "Sal", "Tess", "Vera", "Walt", "Yara", "Zoe", "Marguerite", "Lucien", "Odile", "Remy", "Sylvie"],
  adj: ["Little", "Good", "Golden", "Blue", "Quiet", "Early", "Corner", "Wild", "Copper", "Velvet", "Salt", "Paper", "Honey",
    "Cedar", "Juniper", "Sparrow", "Lantern", "Willow", "Crooked", "Second", "Sunday", "Bright", "Humble", "Gentle",
    "Clever", "Silver", "Saffron", "Indigo", "Marigold", "Hazel"],
};
// A template's slots: {surname}, {given}, {adj}, and a list's own words in [a|b|c].
const NAMES = {
  coffee: ["The {adj} [Bean|Cup|Kettle|Spoon|Mug]", "{surname}'s Coffee", "{adj} [Bean|Cup] Coffee", "{given} & [Crumb|Kettle|Steam]",
    "{surname} Coffee Co.", "The [Daily|Morning|Slow] Grind"],
  bistro: ["Bistro {given}", "{surname}'s Table", "The {adj} [Table|Hearth|Fork|Plate]", "Chez {given}", "{given} & {given}",
    "{adj} [Salt|Olive|Thyme] Kitchen"],
  bakery: ["{surname}'s Bakery", "The {adj} Loaf", "{adj} Crumb Bakehouse", "{given}'s Bread", "Rise & [Shine|Crumb]"],
  books: ["{surname} Books", "The {adj} Page", "{adj} Shelf Books", "{surname} & {surname}", "[Dog-Eared|Well-Thumbed] Books"],
  greengrocer: ["{surname}'s Market", "{adj} Crate Grocer", "The Green [Basket|Crate|Barrow]", "{given}'s Greens", "[Root|Leaf] & Vine"],
  florist: ["{adj} Bloom", "{given}'s Flowers", "{adj} Stem Florist", "The Flower [Cart|Shed|Room]", "[Petal|Stem] & Twine"],
  wine: ["{surname} Wine", "{adj} Cork", "The Wine [Cellar|Room|Shelf]", "[Vine|Cork] & Barrel"],
  bikes: ["{adj} Spoke Cycles", "{surname} Bicycles", "The Bike [Shed|Shop|Works]", "[Chain|Gear] & Spoke"],
  hardware: ["{surname} Hardware", "{adj} Nail Hardware", "{surname} & Sons", "The Tool [Box|Shed|Rack]"],
  records: ["{adj} Groove Records", "Side [A|B] Records", "{surname}'s Records", "The [Needle|Spin|Crate] Shop"],
  tailor: ["{surname} Tailoring", "{adj} Thread", "{given}'s Alterations", "[Needle|Thimble] & Thread"],
  hairdresser: ["{given}'s Hair", "{adj} Comb Salon", "Studio {given}", "The Hair [Room|Parlor|Lab]"],
  accountant: ["{surname} & {surname}, CPAs", "{surname} Accounting", "{surname} Tax & Books", "{adj} Ledger"],
  architect: ["{surname} {surname} Architects", "Studio {surname}", "{surname} Architecture", "Office of {surname}"],
  design: ["{adj} Studio", "{surname} Design", "Studio {adj}", "{adj} & [Grid|Line|Form]"],
  school: ["The {adj} Schoolhouse", "{adj} Little School", "{given}'s Nursery", "The [Acorn|Sapling|Seedling] School"],
  dentist: ["{surname} Family Dentistry", "{adj} Dental", "{surname} Dental Care", "Smile [Studio|House]"],
  gallery: ["{surname} Gallery", "Gallery {given}", "The {adj} Room", "{surname} Contemporary"],
  yoga: ["{adj} Lotus Yoga", "{adj} Yoga Studio", "The Yoga [Loft|Room|House]", "[Breathe|Still] Yoga"],
  barber: ["{surname}'s Barbershop", "{adj} Razor Barbers", "{given}'s Barber", "The [Chair|Shave|Fade] Room"],
  cleaners: ["{adj} Press Cleaners", "{surname} Dry Cleaning", "The Press Room", "[Spotless|Pressed] Cleaners"],
};
const MAX_LEN = 26;

const pick = (rng, xs) => xs[Math.floor(rng() * xs.length)];

/** A name for a business of this kind, from its grammar, short enough for a sign. */
export function nameFor(biz, rng) {
  const templates = NAMES[biz.id] || [biz.noun.replace(/^./, (c) => c.toUpperCase())];
  for (let tries = 0; tries < 8; tries++) {
    const t = pick(rng, templates);
    const used = new Set();
    const fill = (slot) => {
      // A name doesn't use the same word twice ("Hartwell & Hartwell").
      for (let k = 0; k < 6; k++) { const w = pick(rng, W[slot]); if (!used.has(w)) { used.add(w); return w; } }
      return pick(rng, W[slot]);
    };
    const name = t.replace(/\{(\w+)\}/g, (_, slot) => fill(slot)).replace(/\[([^\]]+)\]/g, (_, opts) => pick(rng, opts.split("|")));
    const words = name.split(/[\s&,.']+/).filter((w) => w.length > 2).map((w) => w.toLowerCase());
    if (name.length <= MAX_LEN && new Set(words).size === words.length) return name;
  }
  return biz.sign;
}

/** A face for a business of this kind. */
export function faceFor(biz, rng) {
  return FACE[pick(rng, BY_BUSINESS[biz.id] || BY_FAMILY[biz.family] || BY_FAMILY.shop)];
}

/** The words as they're lettered in a face: capitals for the faces that want them. */
export const lettered = (text, face) => (face.caps ? text.toUpperCase().replace(/\bCPAS\b/g, "CPAs") : text);

/** A canvas font string for a face at `px`. */
export const cssFont = (face, px) => `${face.style === "italic" ? "italic " : ""}${face.weight} ${px}px "${face.family}"`;

/**
 * Packs signs (each { key, text, face }) into rows of a single-channel atlas: the letters white on clear, so a sign's
 * ink is its instance's colour and the texture costs a quarter of an RGBA one. `measure(text, font)` gives a width
 * in pixels. Returns { width, height, px, rows: [{ key, x, y, w }] } (pixels), shrinking the letters until it fits.
 */
export function packSigns(signs, measure, { width = 4096, maxHeight = 4096, px = 44 } = {}) {
  for (let size = px; size >= 20; size -= 4) {
    const rowH = Math.round(size * 1.32), pad = Math.round(size * 0.4);
    const rows = [];
    let x = 0, y = 0;
    for (const s of signs) {
      const w = Math.ceil(measure(lettered(s.text, s.face), cssFont(s.face, size))) + pad;
      if (x + w > width) { x = 0; y += rowH; }
      rows.push({ key: s.key, x, y, w, h: rowH });
      x += w;
    }
    const height = y + rowH;
    if (height <= maxHeight) return { width, height, px: size, rowH, rows };
  }
  throw new Error("too many signs for one atlas");
}

/** Loads the faces (FontFace API) from `base`; resolves when they're in, or after `timeout` ms regardless. */
export function loadSignFaces(base = "/sparrow/fonts/", timeout = 4000) {
  if (typeof FontFace === "undefined" || typeof document === "undefined") return Promise.resolve(false);
  const loads = FACES.map((f) => {
    const ff = new FontFace(f.family, `url(${base}${f.file}.woff2) format("woff2")`, { weight: String(f.weight), style: f.style });
    return ff.load().then((done) => { document.fonts.add(done); return true; }).catch(() => false);
  });
  return Promise.race([Promise.all(loads).then((ok) => ok.every(Boolean)), new Promise((r) => setTimeout(() => r(false), timeout))]);
}
