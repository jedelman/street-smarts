// The chorus, out loud. Eavesdrop on someone and they say what one of the
// pipeline's opinions found about this site, the way a person would say it
// (plain.js: a line per opinion, every number in it the opinion's own), after
// an opener that only says how the opinion scored it (a love or a gripe, a
// shrug when it abstains). The log keeps the opinion's own words beside it.
// Lines come from /vibe-render/clean_baseline_chorus.json
// (crates/street-smarts-conflict/examples/chorus_of.rs, run by
// scripts/vibe-render.sh on the whole site).
//
// The pure parts (picking a line, tidying it) are tested in Node:
// tools/sparrow/chorus.test.mjs.

/** How an opinion's value sounds. Values are 0-1; null means it abstains. */
export function moodOf(value) {
  if (value == null) return "shrug";
  if (value >= 0.75) return "love";
  if (value <= 0.35) return "gripe";
  return "meh";
}

const OPENERS = {
  love: ["Honestly? I love it.", "Okay, this part works.", "Credit where it's due.", "Now this I like.", "Say what you want, this is good."],
  meh: ["It's... fine?", "Half right, I'd say.", "Could go either way.", "Mm. Partly.", "Not bad, not great."],
  gripe: ["Don't get me started.", "This drives me up the wall.", "It's a miss.", "Who signed off on this?", "Ugh."],
  shrug: ["Hm.", "Eh.", "Search me.", "Hah.", "Well."],
};
// Someone answering in a group: agreeing, or not.
const REPLIES = {
  agree: ["And another thing.", "Right, and", "Same here.", "Exactly. Plus"],
  differ: ["Sure, but", "Come on, though.", "See, I don't know.", "Hang on."],
  shrug: ["Don't look at me.", "No idea, myself.", "Can't help you there."],
};

import { plainFor, plainQuestion } from "./plain.js";

const pick = (rng, xs) => xs[Math.floor(rng() * xs.length)];

/** "Alexander et al., A Pattern Language, Pattern 100 (Pedestrian Street)" -> "P100 Pedestrian Street". */
export function shortName(op) {
  const s = op.source || op.name;
  const pat = /Pattern (\d+) \(([^)]+)\)/.exec(s);
  if (pat) return `P${pat[1]} ${pat[2]}`;
  const paren = /\(([^)]+)\)\s*$/.exec(s);
  const author = /^([A-Z][a-z]+)/.exec(s);
  if (paren && !/^\d{4}$/.test(paren[1])) return author ? `${paren[1]} (${author[1]})` : paren[1];
  const name = op.name.replace(/_/g, " ");
  return name[0].toUpperCase() + name.slice(1);
}

/** The finding, tidied for reading: plurals, counts, dashes; cut to a clause or two. */
export function tidy(says, max = 170) {
  let s = String(says || "")
    .replace(/(\w)\(s\)/g, "$1s")
    .replace(/(\w)\(es\)/g, "$1es")
    .replace(/(\d)\/(\d)/g, "$1 of $2")
    .replace(/\s+--\s+/g, " — ")
    .replace(/\s+/g, " ")
    .trim();
  // Clauses end at a full stop or a semicolon followed by a space (a decimal
  // point has none); keep whole ones.
  let out = "";
  for (const p of s.split(/(?<=[.;])\s+/)) {
    const next = (out ? out + " " : "") + p.trim();
    if (out && next.length > max) break;
    out = next;
  }
  out = out.replace(/[;,:]\s*$/, "").trim();
  // One clause can run long (an abstention explaining itself): stop at a word.
  if (out.length > max * 1.5) return out.slice(0, max * 1.5).replace(/[\s,;:—(]+\S*$/, "") + "…";
  return /[.!?]$/.test(out) ? out : out + ".";
}

/** The same line for the ear: units and symbols said in words, ids not read out. */
export function forSpeech(text) {
  return text
    .replace(/\b(?:p\d+_|MILITARY_CIRCLE_)\w+/g, "one of them")
    .replace(/(\d)\s*m²/g, "$1 square metres")
    .replace(/(\d)\s*m\b/g, "$1 metres")
    .replace(/(\d)\s*ft\b/g, "$1 feet")
    .replace(/(\d)\s*yd\b/g, "$1 yards")
    .replace(/(\d)×/g, "$1 times")
    .replace(/\s*→\s*/g, " to ")
    .replace(/\s*(?:>=|≥)\s*/g, " at least ")
    .replace(/\s*(?:<=|≤)\s*/g, " at most ")
    .replace(/~/g, "about ")
    .replace(/_/g, " ");
}

/**
 * Which opinion someone voices next: one not heard yet, a few times likelier
 * if it looked at something near them (`near`, a Set of element source ids),
 * and, when `against` is a mood, likelier to see it the other way.
 */
export function pickOpinion(chorus, { near = new Set(), heard = new Set(), against = null, rng = Math.random } = {}) {
  let pool = chorus.opinions.filter((o) => !heard.has(o.name));
  if (!pool.length) { heard.clear(); pool = chorus.opinions.slice(); }
  const opposite = { love: ["gripe", "meh"], gripe: ["love", "meh"], meh: ["love", "gripe"], shrug: ["love", "gripe"] };
  const weights = pool.map((o) => {
    const mood = moodOf(o.value);
    let w = mood === "shrug" ? 0.35 : 1;
    if (near.size && (o.features || []).some((f) => near.has(f))) w *= 4;
    if (against && opposite[against].includes(mood)) w *= 5;
    return w;
  });
  let r = rng() * weights.reduce((a, b) => a + b, 0);
  for (let i = 0; i < pool.length; i++) { r -= weights[i]; if (r <= 0) return pool[i]; }
  return pool[pool.length - 1];
}

/**
 * A line for opinion `op`. `after` is the mood of the line it answers, if
 * it's a reply in a group. Returns { mood, opener, said, finding, plain, text,
 * speech, cite }: `said` is what the person says (the plain line, or the
 * opinion's own words when it has none), `finding` the opinion's own words.
 */
export function lineFor(op, { rng = Math.random, after = null } = {}) {
  const mood = moodOf(op.value);
  let opener;
  if (after) {
    if (mood === "shrug") opener = pick(rng, REPLIES.shrug);
    else opener = pick(rng, mood === after ? REPLIES.agree : REPLIES.differ);
  } else opener = pick(rng, OPENERS[mood]);
  const finding = tidy(op.says);
  const plainLine = plainFor(op);
  const said = plainLine || finding;
  // "Sure, but" runs straight on into the line: "Sure, but the rooms are...".
  const runOn = opener && !/[.?!]$/.test(opener) && !/^(I\b|I'|Alexander|Salingaros)/.test(said);
  const text = `${opener} ${runOn ? said[0].toLowerCase() + said.slice(1) : said}`;
  const score = op.value == null ? "no view" : op.value.toFixed(2);
  return { mood, opener, said, finding, plain: !!plainLine, text, speech: forSpeech(text), cite: `${shortName(op)} · ${score}` };
}

/** The chorus's own question for the humans in it, said by a bystander. */
export function questionLine(chorus, rng = Math.random) {
  const qs = chorus.questions || [];
  if (!qs.length) return null;
  const q = pick(rng, qs);
  const plain = plainQuestion(q.question);
  const said = plain || q.question;
  return { mood: "question", opener: "", said, finding: q.question, plain: !!plain, text: said, speech: forSpeech(said), cite: "The chorus's question for humans" };
}

/**
 * The browser's own voices (the Web Speech API), a different one and a
 * different pitch per person. Silent when muted or when there are none; the
 * caller shows the words either way. say() calls `done` once, when the line
 * has been said (or would have been).
 */
export function makeVoices() {
  const synth = globalThis.speechSynthesis;
  let voices = [];
  const load = () => {
    if (!synth) return;
    const all = synth.getVoices();
    voices = all.filter((v) => /^en(-|_|$)/i.test(v.lang));
    if (!voices.length) voices = all;
  };
  load();
  if (synth && synth.addEventListener) synth.addEventListener("voiceschanged", load);
  let token = 0;
  return {
    say(text, seed, { muted = false, volume = 1, done = () => {} } = {}) {
      const mine = ++token;
      let finished = false;
      const finish = () => { if (!finished && mine === token) { finished = true; done(); } };
      // How long reading it takes: the pause when there's no voice, and a fallback (onend can fail to fire).
      const est = 900 + text.length * 62;
      if (!voices.length) load();
      if (!synth || muted || !voices.length || typeof SpeechSynthesisUtterance === "undefined") { setTimeout(finish, est); return est; }
      synth.cancel();
      const u = new SpeechSynthesisUtterance(text);
      u.voice = voices[seed % voices.length];
      u.volume = volume;
      u.pitch = 0.75 + ((seed * 37) % 60) / 100;
      u.rate = 0.95 + ((seed * 13) % 20) / 100;
      u.onend = finish;
      u.onerror = finish;
      synth.speak(u);
      setTimeout(finish, est * 1.8 + 2500);
      return est;
    },
    stop() { token++; if (synth) synth.cancel(); },
  };
}
