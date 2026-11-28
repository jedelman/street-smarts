// See-through: whatever stands between the camera and the sparrow thins out, rather than the camera being shoved
// in front of it. The camera keeps its place on its boom (no more jumps when a wall, a curb or a porch post crosses
// the line), and the surfaces in the way turn translucent by their normals: a face turned square to the camera
// all but vanishes, one seen edge-on stays, so a wall in front of you reads as the outline of a building.
//
// Translucency is a screen-door dither (discard by a per-pixel threshold), so it needs no sorting and keeps depth.
// It applies inside a cone from the camera to the sparrow (a clear circle on screen around you) and to anything
// almost touching the lens. Surfaces lying flat below the sparrow (a walk, a step, a floor) never thin: they're
// what you stand on, not what's in the way. Shadows aren't affected: the depth pass doesn't run this.
import * as THREE from "three";

/** Shared uniforms: main.js updates them once a frame (update below). */
export const see = {
  uSeeAt: { value: new THREE.Vector3() }, // the sparrow's head, in view space
  uSeeUp: { value: new THREE.Vector3(0, 1, 0) }, // world up, in view space
  uSeeOn: { value: 0 },
  uSeeR: { value: Math.tan(THREE.MathUtils.degToRad(17)) }, // the clear circle's angular radius
};

const VERT = "\nvarying vec3 vSeeView;";
const FRAG = `
varying vec3 vSeeView;
uniform vec3 uSeeAt, uSeeUp;
uniform float uSeeOn, uSeeR;
float seeNoise(vec2 p) { return fract(52.9829189 * fract(dot(p, vec2(0.06711056, 0.00583715)))); }
`;
// How much of a surface to keep, from 1 (all of it) down: `fade` says how far it's in the way, its normal how much.
const TEST = `
if (uSeeOn > 0.5) {
  vec3 seeN = normalize(cross(dFdx(vSeeView), dFdy(vSeeView))); // the face's own normal, for any material
  float seeD = length(vSeeView);
  float seeTz = length(uSeeAt);
  vec3 seeDir = uSeeAt / max(seeTz, 1e-4);
  float seeAlong = dot(vSeeView, seeDir);
  float seePerp = length(vSeeView - seeDir * seeAlong);
  float seeR = max(seeAlong, 0.0) * uSeeR;
  float seeFade = (1.0 - smoothstep(seeR * 0.7, seeR, seePerp)) * (1.0 - smoothstep(seeTz - 0.55, seeTz - 0.25, seeAlong)) * step(0.0, seeAlong);
  seeFade = max(seeFade, 1.0 - smoothstep(0.35, 0.9, seeD));
  float seeFlatBelow = step(0.75, abs(dot(seeN, uSeeUp))) * step(dot(vSeeView - uSeeAt, uSeeUp), 0.2);
  seeFade *= 1.0 - seeFlatBelow;
  float seeFacing = abs(dot(seeN, vSeeView / max(seeD, 1e-4)));
  float seeKeep = mix(1.0, 0.1 + 0.9 * (1.0 - seeFacing) * (1.0 - seeFacing), seeFade);
  if (seeKeep < 0.999 && seeKeep <= seeNoise(gl_FragCoord.xy)) discard;
}
`;

/** Make a material see-through (idempotent; chains any onBeforeCompile it already has). */
export function seeThrough(material) {
  if (!material || material.userData.see) return material;
  material.userData.see = true;
  const prev = material.onBeforeCompile;
  const key = material.customProgramCacheKey();
  material.onBeforeCompile = function (shader, renderer) {
    if (prev) prev.call(this, shader, renderer);
    Object.assign(shader.uniforms, see);
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", `#include <common>${VERT}`)
      .replace("#include <project_vertex>", "#include <project_vertex>\nvSeeView = mvPosition.xyz;");
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", `#include <common>${FRAG}`)
      .replace("#include <clipping_planes_fragment>", `#include <clipping_planes_fragment>${TEST}`);
  };
  // Programs are cached by this key: keep the material's own (its onBeforeCompile's source) and mark ours.
  material.customProgramCacheKey = () => `${key}|see1`;
  material.needsUpdate = true;
  return material;
}

/** Materials that are drawn to depth (not a decal or a glow laid over something): the ones worth thinning. */
export function solid(material) {
  if (!material) return false;
  if (material.blending === THREE.AdditiveBlending) return false;
  return !material.transparent || material.alphaTest > 0;
}

const _v = new THREE.Vector3();
/** Once a frame, after the camera has moved: where the sparrow is, seen from it. */
export function update(camera, head, on) {
  see.uSeeOn.value = on ? 1 : 0;
  if (!on) return;
  camera.updateMatrixWorld();
  see.uSeeAt.value.copy(head).applyMatrix4(camera.matrixWorldInverse);
  see.uSeeUp.value.copy(_v.set(0, 1, 0)).transformDirection(camera.matrixWorldInverse);
}
