// The site map: the walk's own scene, rendered once from straight above into an
// image, so the plan you see is the plan you're flying through (buildings,
// streets, squares, trees, the sculptures). The sparrow is drawn on it each
// frame the map is open; a tap on it gives back the place in the world.
import * as THREE from "three";

/**
 * Renders `box` (a Box3 in world space) of `scene` from above, `px` pixels on its
 * long side, with fog off. Returns { image (a canvas), toMap(x, z) -> [u, v] in
 * 0..1, toWorld(u, v) -> { x, z } }.
 */
export function renderPlan(renderer, scene, box, px = 900) {
  const c = box.getCenter(new THREE.Vector3()), s = box.getSize(new THREE.Vector3());
  const w = Math.max(10, s.x), h = Math.max(10, s.z);
  const W = w >= h ? px : Math.round(px * w / h), H = w >= h ? Math.round(px * h / w) : px;
  const cam = new THREE.OrthographicCamera(-w / 2, w / 2, h / 2, -h / 2, 1, box.max.y - box.min.y + 400);
  cam.position.set(c.x, box.max.y + 200, c.z);
  cam.up.set(0, 0, -1); // north (-z) at the top
  cam.lookAt(c.x, box.min.y, c.z);
  cam.updateMatrixWorld();
  const target = new THREE.WebGLRenderTarget(W, H, { colorSpace: renderer.outputColorSpace });
  const fog = scene.fog;
  scene.fog = null;
  const prev = renderer.getRenderTarget();
  renderer.setRenderTarget(target);
  renderer.render(scene, cam);
  const buf = new Uint8Array(W * H * 4);
  renderer.readRenderTargetPixels(target, 0, 0, W, H, buf);
  renderer.setRenderTarget(prev);
  scene.fog = fog;
  target.dispose();
  // Rows come bottom-up; flip them into the canvas.
  const image = document.createElement("canvas");
  image.width = W; image.height = H;
  const ctx = image.getContext("2d"), data = ctx.createImageData(W, H);
  for (let y = 0; y < H; y++) data.data.set(buf.subarray((H - 1 - y) * W * 4, (H - y) * W * 4), y * W * 4);
  ctx.putImageData(data, 0, 0);
  const x0 = c.x - w / 2, z0 = c.z - h / 2;
  return {
    image,
    toMap: (x, z) => [(x - x0) / w, (z - z0) / h],
    toWorld: (u, v) => ({ x: x0 + u * w, z: z0 + v * h }),
  };
}
