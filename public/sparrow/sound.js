// Little sounds, synthesized: no audio files. Starts on the first tap or key
// (browsers allow sound only after a gesture). M mutes; remembered on this
// device. After Dream of the Earth's sound.js.

let ctx = null, master = null, noiseBuf = null, unlocked = false, muted = false;
try { muted = localStorage.getItem("street-smarts-muted") === "1"; } catch { /* not remembered */ }

function audio() {
  if (muted || !unlocked) return null;
  if (!ctx) {
    const AC = globalThis.AudioContext || globalThis.webkitAudioContext;
    if (!AC) return null;
    ctx = new AC();
    master = ctx.createGain();
    master.gain.value = 0.28;
    master.connect(ctx.destination);
    noiseBuf = ctx.createBuffer(1, ctx.sampleRate, ctx.sampleRate);
    const d = noiseBuf.getChannelData(0);
    let x = 12345;
    for (let k = 0; k < d.length; k++) { x = (x * 1103515245 + 12345) >>> 0; d[k] = (x / 2 ** 31) - 1; }
  }
  if (ctx.state === "suspended") ctx.resume().catch(() => {});
  return ctx;
}
function env(g, t, peak, attack, release) {
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(peak, t + attack);
  g.gain.exponentialRampToValueAtTime(0.0001, t + attack + release);
}
function tone(type, f0, f1, t, dur, peak) {
  const o = ctx.createOscillator(), g = ctx.createGain();
  o.type = type;
  o.frequency.setValueAtTime(f0, t);
  o.frequency.exponentialRampToValueAtTime(f1, t + dur);
  env(g, t, peak, 0.006, dur);
  o.connect(g).connect(master);
  o.start(t); o.stop(t + dur + 0.05);
}
function noise(t, dur, peak, type, f0, f1, q = 1) {
  const s = ctx.createBufferSource(), f = ctx.createBiquadFilter(), g = ctx.createGain();
  s.buffer = noiseBuf; s.loop = true;
  f.type = type; f.Q.value = q;
  f.frequency.setValueAtTime(f0, t);
  f.frequency.exponentialRampToValueAtTime(f1, t + dur);
  env(g, t, peak, 0.01, dur);
  s.connect(f).connect(g).connect(master);
  s.start(t); s.stop(t + dur + 0.1);
}
const PENTA = [523.25, 587.33, 659.25, 783.99, 880, 1046.5];

export const sound = {
  get muted() { return muted; },
  unlock() { unlocked = true; audio(); },
  toggle() {
    muted = !muted;
    try { localStorage.setItem("street-smarts-muted", muted ? "1" : "0"); } catch { /* not remembered */ }
    if (muted && ctx) ctx.suspend(); else audio();
    return muted;
  },
  /** Two or three quick rising-falling notes. */
  chirp() {
    if (!audio()) return;
    const t = ctx.currentTime, n = 2 + (Math.random() < 0.4 ? 1 : 0);
    for (let i = 0; i < n; i++) {
      const f = 3200 + Math.random() * 1400;
      tone("sine", f, f * 1.35, t + i * 0.09, 0.06, 0.18);
      tone("sine", f * 1.3, f * 0.9, t + i * 0.09 + 0.035, 0.05, 0.1);
    }
  },
  hop() { if (audio()) noise(ctx.currentTime, 0.03, 0.05, "highpass", 2500, 4000, 0.7); },
  flap() { if (audio()) noise(ctx.currentTime, 0.12, 0.09, "bandpass", 600, 1400, 1.2); },
  land() { if (audio()) noise(ctx.currentTime, 0.06, 0.08, "lowpass", 900, 300, 0.8); },
  /** A soft bell: markers answer in pentatonic notes. */
  chime(i = 0) {
    if (!audio()) return;
    const t = ctx.currentTime, f = PENTA[((i % PENTA.length) + PENTA.length) % PENTA.length];
    tone("sine", f, f * 0.998, t, 1.4, 0.14);
    tone("sine", f * 2.01, f * 2, t, 0.7, 0.04);
  },
  rustle() { if (audio()) noise(ctx.currentTime, 0.35, 0.07, "bandpass", 3000, 1800, 0.6); },
  coo() {
    if (!audio()) return;
    const t = ctx.currentTime;
    tone("sine", 330, 280, t, 0.22, 0.08);
    tone("sine", 300, 260, t + 0.25, 0.3, 0.06);
  },
  flutter() { if (audio()) for (let i = 0; i < 4; i++) noise(ctx.currentTime + i * 0.05, 0.06, 0.06, "bandpass", 900, 1600, 1.5); },
  sit() { if (audio()) tone("sine", 900, 700, ctx.currentTime, 0.12, 0.05); },
};
