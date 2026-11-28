// Ghent Streets' Place panel: the last lot you touched, with the City's own record of it. The words are the
// model's (tools/ghent/build.py writes them from the survey and the proposal); the photo is the City of Norfolk
// assessor's, shown from the City's server and credited to it.
export function render(el, { place }) {
  el.innerHTML = "";
  const f = place && place.facts;
  if (!f) {
    el.innerHTML = "<p>Touch a building and its lot shows here: what it is, what its ground floor could hold, and why it may.</p>";
    return;
  }
  const h = document.createElement("h3");
  h.textContent = f.name;
  el.append(h);
  if (f.photo) {
    const fig = document.createElement("figure");
    fig.className = "photo";
    const img = document.createElement("img");
    img.src = f.photo;
    img.loading = "lazy";
    img.referrerPolicy = "no-referrer";
    img.alt = `${f.name}, photographed from the street by the City of Norfolk's assessor.`;
    const cap = document.createElement("figcaption");
    cap.textContent = "Photo: City of Norfolk Real Estate Assessor";
    fig.append(img, cap);
    el.append(fig);
  }
  for (const line of f.answer || []) {
    const p = document.createElement("p");
    p.textContent = line;
    el.append(p);
  }
  const links = [];
  if (f.case) links.push(["Read the case study", f.case]);
  if (f.card) links.push(["The assessor's property card", f.card]);
  if (f.streetview) links.push(["See it on Street View", f.streetview]);
  if (links.length) {
    const p = document.createElement("p");
    p.className = "links";
    links.forEach(([text, href], i) => {
      const a = document.createElement("a");
      a.href = href;
      a.target = "_blank";
      a.rel = "noopener";
      a.textContent = `${text} ↗`;
      if (i) p.append(" · ");
      p.append(a);
    });
    el.append(p);
  }
  if (f.status === "converted") {
    const p = document.createElement("p");
    p.className = "small";
    p.textContent = "The business, its sign and what it puts out are ours, one of the uses the amendment would permit. The doors and windows are where the model infers them.";
    el.append(p);
  }
}

/** In context (main.js surface): touching a lot the assessor photographed, its photo and the City's record. */
export function peek({ touched, reason }) {
  const f = reason === "touch" && touched && touched.facts;
  if (!f || !f.photo) return null;
  return { key: `place:${touched.source}`, text: `The assessor's photo of ${f.name}, with the City's record of it.` };
}
