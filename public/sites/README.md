# Sites

street-smarts is one walk and as many ways of making a place as a project needs. Each place is a **site**, and
each site is a bundle: a page, a `site.json`, and the modules that render its own panels.

| Site | Page | Pipeline | Money |
|---|---|---|---|
| Eastside Commons (Military Circle) | `/` (`public/index.html`) | patterns: the Rust pipeline invents the plan; `tools/vibe-render` renders it | greenfield: who pays to build it (`eastside-commons/economics.js`) |
| Ghent Streets (the Ghent Neighborhood League's area) | `/ghent/` | adaptive reuse: `tools/ghent` surveys the City's GIS (`survey.py`), proposes which ground floors open and what they hold (`propose.py`), and builds the model (`build.py`) | reuse: what opening ground floors brings the city (`ghent-streets/money.js`, quoting Policy Brief No. 2) |

## The contract

**The model** (`.glb`) holds what belongs to places in the world, so any glTF viewer keeps it:
- geometry, packed one mesh per material (`tools/vibe-render/pack_glb.py`);
- the root node's `extras.streetSmarts`: every element (`group`, `kind`, `source`, `typology`, `floors`, `uses`,
  `area`), and the detail (walls, openings, shop fronts), trees and transit a pipeline adds;
- `facts`: the words attached to a place, by source id (`"parcel:<gpin>": { name, prompt, answer: [lines],
  arrival, photo, card, streetview, case }`), or a group's template (`"group:trees"`, filled from the element's
  fields: `{typology|fallback}`, `{floors?text with #}`). The walk shows whatever facts an element carries;
- waypoints as glTF camera nodes (`extras: { waypoint, source, title, look }`), so a tour's stops travel with the
  model and any glTF viewer can show them as viewpoints;
- shop rows a pipeline typed (`typology`, `business`): a `storefront` replaces the ground floor's openings, any
  other typology keeps the building's own doors and windows and lights them with the new use.

**`site.json`** holds what the walk reads but isn't a place:
- `model`, and what's published beside it: `numbers` (a pipeline's measurements), `chorus`, `archive`;
- `tours`: each a title, a note and stops. A stop is `{ "pattern": n }` (the nearest place that pattern made, for
  a generated plan whose elements change every render), `{ "element": "<source id>" }`, or
  `{ "waypoint": "<id>" }` (a waypoint the model carries: where to stand, what to look at). One tour starts from
  the button; several open a list;
- `renderers`: `{ "<panel>": "<module>" }`. The walk imports the module the first time that panel opens and calls
  `render(el, { numbers, site, elements, flyTo, goToElement })` into `#<panel>-body`.

**The page** holds the site's words: its intro, its panels' prose, its help. It names its bundle with
`<meta name="street-smarts-site" content="/sites/<id>/site.json">`, links `/sparrow/walk.css`, and loads
`/sparrow/main.js`. Prose stays in HTML so people can read, edit and check it.

Interactions are the walk's verbs (touch, follow, fly to, go inside); a site says which apply to what, and never
carries behaviour in the model.
