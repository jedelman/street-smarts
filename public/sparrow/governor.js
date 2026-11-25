// The resolution governor: each device finds the sharpest picture it can hold
// at a steady frame rate. Every couple of seconds it looks at the median frame
// interval and lowers the render scale (a pixel ratio) when frames run slow,
// or raises it when the display's own rate is being met. After Dream of the
// Earth's detail governor, with one addition: after it steps down it won't
// climb back past the scale that was too slow for about thirty seconds, so it
// settles instead of see-sawing (each change reallocates the framebuffer, a
// small hitch).
// Pure, so Node can test it.

export const EASY_MS = 18, SLOW_MS = 22, VERY_SLOW_MS = 40; // meeting 60 Hz; under ~45 fps; under 25

/** The median of a list of numbers (frame times), robust to the odd hitch. */
export function median(xs) {
  if (!xs.length) return 0;
  const s = [...xs].sort((a, b) => a - b), m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

/**
 * Where a device starts and how far it may go. Phones and tablets start at 1
 * and stop at 1.5 (dense small screens; the battery matters); desktops start
 * at up to 1.25 and may reach their screen's own ratio, capped at 2.
 */
export function scaleRange(devicePixelRatio, coarse) {
  const dpr = Math.max(1, devicePixelRatio || 1);
  if (coarse) return { start: 1, min: 0.7, max: Math.min(dpr, 1.5) };
  return { start: Math.min(dpr, 1.25), min: 0.6, max: Math.min(dpr, 2) };
}

export function createGovernor({ start, min, max }) {
  let scale = start, ceiling = max, calm = 0;
  return {
    get scale() { return scale; },
    get ceiling() { return ceiling; },
    /** Feed the median frame interval (ms) of the last window; returns the scale to use. */
    step(medianMs) {
      let next = scale;
      if (medianMs > VERY_SLOW_MS) next = scale * 0.7;
      else if (medianMs > SLOW_MS) next = scale * 0.85;
      else if (medianMs < EASY_MS) next = scale * 1.1;
      if (next < scale) { ceiling = Math.max(min, scale * 0.95); calm = 0; }
      else if (++calm > 15) ceiling = Math.min(max, ceiling * 1.02); // after ~30 s of calm, forgive slowly
      next = Math.min(Math.min(max, ceiling), Math.max(min, next));
      if (Math.abs(next - scale) >= 0.02) scale = Math.round(next * 100) / 100;
      return scale;
    },
  };
}
