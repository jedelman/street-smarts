// Camera maths: the pieces a camera operator does by feel. Pure, so Node can
// test it. smoothDamp, shotWeight and bankFor come from Dream of the Earth's
// camerawork.js (jedelman/dream-of-the-earth), where they were tuned by hand.

/**
 * Critically damped spring toward a target (the classic SmoothDamp): fast to
 * start, no overshoot, arriving in about `smoothTime` seconds. Returns [value, velocity].
 */
export function smoothDamp(current, target, velocity, smoothTime, dt) {
  const omega = 2 / Math.max(1e-4, smoothTime), x = omega * dt;
  const decay = 1 / (1 + x + 0.48 * x * x + 0.235 * x * x * x);
  const change = current - target;
  const temp = (velocity + omega * change) * dt;
  return [target + (change + temp) * decay, (velocity - omega * temp) * decay];
}

/** Shortest signed angle from a to b (radians). */
export const angleTo = (a, b) => Math.atan2(Math.sin(b - a), Math.cos(b - a));

/** smoothDamp for an angle: goes the short way round. */
export function smoothDampAngle(current, target, velocity, smoothTime, dt) {
  return smoothDamp(current, current + angleTo(current, target), velocity, smoothTime, dt);
}

const smooth01 = (t) => { const c = Math.min(1, Math.max(0, t)); return c * c * (3 - 2 * c); };

/** How much an event shot counts `age` seconds in: eases in, holds, eases out by `dur`. */
export function shotWeight(age, dur, blendIn = 0.6, blendOut = 1) {
  if (age < 0 || age > dur) return 0;
  const inW = blendIn > 0 ? smooth01(age / blendIn) : 1;
  const outW = blendOut > 0 ? smooth01((dur - age) / blendOut) : 1;
  return Math.min(inW, outW);
}

/** Lean into a turn (radians of roll): more at speed, capped at 12°. turnRate in rad/s. */
export function bankFor(turnRate, speed01) {
  return Math.max(-0.21, Math.min(0.21, -turnRate * 0.3 * speed01)) + 0; // + 0: no negative zero
}

/** Field of view (degrees) for how fast you're going: a little wider at speed. */
export const fovFor = (speed01) => 60 + 12 * speed01;
