// The site is static assets (public/, served by Workers Assets before this
// script runs). The one thing this adds: /archive/*, the plan's past
// iterations, from the R2 bucket bound as ARCHIVE. Each iteration is a
// folder of its own (archive/<id>/clean_baseline.glb, numbers.json,
// chorus.json, isometric.png) and archive/index.json lists them. Uploaded by
// tools/vibe-render/archive_milestone.py; never written from here.
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (!url.pathname.startsWith("/archive/")) return env.ASSETS.fetch(request);
    if (request.method !== "GET" && request.method !== "HEAD") return new Response("Method not allowed", { status: 405 });
    const key = decodeURIComponent(url.pathname.slice(1));
    if (key.includes("..")) return new Response("Not found", { status: 404 });
    const obj = await env.ARCHIVE.get(key);
    if (!obj) return new Response("Not found", { status: 404 });
    const headers = new Headers();
    obj.writeHttpMetadata(headers);
    headers.set("etag", obj.httpEtag);
    // An iteration never changes once archived; the list of them does.
    headers.set("cache-control", key.endsWith("/index.json") ? "public, max-age=300" : "public, max-age=31536000, immutable");
    return new Response(request.method === "HEAD" ? null : obj.body, { headers });
  },
};
