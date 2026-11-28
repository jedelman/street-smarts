// The light and the land around the model: a golden-hour sky drawn by one
// cheap shader (no HDR file to fetch, no PMREM to compute), a sun and a sky
// light, fog that fades the far context into the horizon, and a plain ground
// so the model stands on something. None of it is data.
import * as THREE from "three";

export const SKY = {
  zenith: new THREE.Color("#7aa3d6"),
  horizon: new THREE.Color("#f3d3ab"),
  sun: new THREE.Color("#ffd58f"),
  ground: new THREE.Color("#bdb4a1"),
};
// Low in the west-southwest, the hour the gallery's own HDRI was chosen for.
export const SUN_DIR = new THREE.Vector3(-0.78, 0.32, 0.36).normalize();

/**
 * The sky, the sun and a wide ground disc. opts.floorY: the land's level, where the disc goes, when there's water
 * below it (Ghent's Hague); opts.holes: rings of [x, z, x, z, ...] where the disc stays open over that water.
 */
export function buildWorld(scene, bounds, opts = {}) {
  const size = bounds.getSize(new THREE.Vector3());
  const center = bounds.getCenter(new THREE.Vector3());
  const span = Math.max(size.x, size.z);

  scene.background = SKY.horizon.clone();
  scene.fog = new THREE.Fog(SKY.horizon, span * 0.35, span * 1.6);

  const hemi = new THREE.HemisphereLight("#d7e4ff", "#a8916f", 1.9);
  scene.add(hemi);
  const sun = new THREE.DirectionalLight("#ffe0b5", 2.4);
  sun.position.copy(SUN_DIR);
  scene.add(sun);

  // Sky dome: drawn at the far plane, follows the camera.
  const sky = new THREE.Mesh(
    new THREE.SphereGeometry(1, 24, 12),
    new THREE.ShaderMaterial({
      uniforms: {
        uZenith: { value: SKY.zenith }, uHorizon: { value: SKY.horizon }, uSun: { value: SKY.sun },
        uSunDir: { value: SUN_DIR }, uTime: { value: 0 },
      },
      vertexShader: /* glsl */ `
        varying vec3 vDir;
        void main() {
          vDir = position;
          vec4 p = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
          gl_Position = p.xyww; // on the far plane
        }`,
      fragmentShader: /* glsl */ `
        uniform vec3 uZenith, uHorizon, uSun, uSunDir;
        varying vec3 vDir;
        void main() {
          vec3 d = normalize(vDir);
          float h = max(d.y, 0.0);
          vec3 col = mix(uHorizon, uZenith, pow(h, 0.55));
          float s = max(dot(d, uSunDir), 0.0);
          col += uSun * (pow(s, 900.0) * 3.0 + pow(s, 24.0) * 0.28 + pow(s, 4.0) * 0.08);
          col = mix(col, uHorizon, smoothstep(0.0, -0.08, d.y)); // below the horizon: haze
          gl_FragColor = vec4(col, 1.0);
          #include <colorspace_fragment>
        }`,
      side: THREE.BackSide, depthWrite: false, depthTest: false, fog: false,
    }),
  );
  sky.renderOrder = -10;
  sky.frustumCulled = false;
  scene.add(sky);

  // Ground: a wide disc just under the model, fading into the fog; open where there's water below the land.
  const floorY = opts.floorY ?? bounds.min.y;
  let discGeo;
  if (opts.holes && opts.holes.length) {
    const disc = new THREE.Shape();
    disc.absarc(0, 0, span * 4, 0, Math.PI * 2, false);
    for (const ring of opts.holes) {
      const hole = new THREE.Path();
      for (let i = 0; i < ring.length; i += 2) {
        const x = ring[i] - center.x, y = -(ring[i + 1] - center.z); // the shape's y is the world's -z
        if (i === 0) hole.moveTo(x, y); else hole.lineTo(x, y);
      }
      disc.holes.push(hole);
    }
    discGeo = new THREE.ShapeGeometry(disc, 48).rotateX(-Math.PI / 2);
  } else discGeo = new THREE.CircleGeometry(span * 4, 64).rotateX(-Math.PI / 2);
  const ground = new THREE.Mesh(discGeo, new THREE.MeshLambertMaterial({ color: SKY.ground }));
  ground.position.set(center.x, floorY - 0.02, center.z);
  scene.add(ground);

  return {
    sky, sun, hemi, ground, groundY: bounds.min.y,
    /** Where to stand when nothing of the model is below: the disc (the land), not the water's level. */
    floorY,
    follow(camera) { sky.position.copy(camera.position); sky.scale.setScalar(camera.far * 0.9); },
  };
}

/** A soft round shadow texture for the bird's blob shadow. */
export function blobTexture() {
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d");
  const grad = g.createRadialGradient(32, 32, 0, 32, 32, 32);
  grad.addColorStop(0, "rgba(40,30,20,0.55)");
  grad.addColorStop(0.6, "rgba(40,30,20,0.25)");
  grad.addColorStop(1, "rgba(40,30,20,0)");
  g.fillStyle = grad;
  g.fillRect(0, 0, 64, 64);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}
