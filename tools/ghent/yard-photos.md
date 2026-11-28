# How Ghent's fronts look: the assessor's photos, tallied

The yard and porch grammar (`public/sparrow/yards.js`) and the house wall palette (`tools/ghent/build.py`) draw
their weights from this tally. The source is the City of Norfolk Real Estate Assessor's own street photos of the
neighborhood (gisshare.norfolk.gov), the same photos the Lot panel shows. Most are dated 2025–2026.

## The photos (2026-11-28)

| Set | Photos | Usable | Note |
|---|---|---|---|
| The model's corner lots and tour stops | 77 | 74 | Almost all corner lots, so apartments are over-represented. |
| A random sample of mid-block houses | 48 | 47 | Seed 1128, drawn from single-family, duplex, triplex and fourplex parcels that aren't corners. |
| **Total** | **125** | **121** | 73 houses (detached or semi), 9 rowhouses, 32 apartment buildings, 7 other. |

Three readers (Claude Code subagents) each catalogued one batch on the same fixed schema, one record per photo:
porch, columns, railing, porch roof, steps, shutters, setback, yard edge, walk, ground, foundation planting, plants,
yard trees, containers, porch furniture, decorations, facade extras, and wall, trim and door colours. They recorded
buildings and yards only, never people or cars. The per-photo catalogues aren't published; these are the aggregates.
The readers' main uncertainties: shrub species (boxwood, holly and privet look alike at this resolution), flat versus
low hip porch roofs seen head-on, and yard trees versus street trees at the edge of the frame.

## Houses (detached and semi-detached; n = 73)

- **Porch** (n = 71): full-width 41%, partial 32%, wraparound 7%, stoop 18%, none 1%. Two-level porches: 3%.
- **Columns** (n = 53): round classical 70%, square 13%, brick piers 11%, turned 4%, tapered on piers 2%.
- **Railing** (n = 55): wood balusters 56%, iron 25%, none 13%, brick knee wall 5%.
- **Porch roof** (n = 51): hip 53%, flat 16%, gable 14%, shed 10%, flat with balustrade 8%.
- **Steps:** about 5 on average (4.8). Material (n = 46): brick 50%, concrete 39%, stone 7%, wood 4%.
- **Setback** (n = 69): medium (3–8 m) 59%, shallow 36%, deep 4%.
- **Yard edge** (n = 61): none 72%, hedge 11%, iron fence 7%, low brick wall 5%, wood picket 3%, stone wall 2%.
- **Walk** (n = 28 where visible): concrete 50%, brick 32%, none 18%.
- **Ground** (n = 58, several at once): mulch beds 81%, lawn 47%, ivy or liriope 10%.
- **Foundation planting** (n = 72): continuous 64%, sparse 36%.
- **Plants** (all photos): clipped evergreen shrubs the readers couldn't name in 83 photos, boxwood in 40, crape myrtle
  16, Japanese maple 12, magnolia 9, liriope 7, ornamental grass 6, holly 6, azalea 5, ferns 5.
- **Yard trees:** 72% of houses have at least one (1.15 on average). Named kinds: Japanese maple and crape myrtle
  (10 each), southern magnolia (7), conical evergreens or holly (about 7), Italian cypress (2), and small deciduous
  trees the readers couldn't name (about 14).
- **Shutters:** 27% (black 45% of those).
- **Containers:** pots on steps 32%, planters on the porch 21%, urns 11%, hanging baskets 10%.
- **Porch furniture:** chairs 23%, a table 8%, a bench 5%, rocking chairs 3%.
- **Decorations:** a house-number plaque 38%, a flag 16%, art or sculpture 10%, a wreath 7%.
- **Facade:** dormers 59%, lanterns by the door 48%, a transom 38%, sidelights 18%, a pediment 16%, a bay window 16%.
- **Walls:** brick 47%; two-tone 32% (mostly a brick ground floor with shingle or siding above); lap siding 12%;
  shingle 5%; stucco 4%.
- **Trim:** white 74%, dark brown 7%, cream 5%, dark green 3%. **Doors** (n = 51): white 24%, black 20%, natural or
  dark wood 28%, red 8%, navy 4%, teal 2%.

## Rowhouses (n = 9: small, so read loosely)

Stoop 67%, a porch 33%. Railings mostly iron (5 of 8). Shallow setbacks (8 of 9). Edge: none 4 of 7, then an iron
fence, a hedge or a low wall. Planting sparse (6 of 9). Doors black (6 of 9). Shutters 1 of 9.

## Apartment buildings (n = 32)

- **Porch** (n = 30): partial 47%, full-width 13%, stoop 27%, none 13%. Half the porches (13 of 26) are stacked two or
  more storeys; several are three or four tiers.
- **Columns** (n = 14): round 36%, brick piers 36%, square 29%. **Railing** (n = 18): iron 56%, wood balusters 39%.
- **Porch roof** (n = 17): flat 65%, gable 18%, shed 12%.
- **Steps:** concrete 75%. **Walk:** concrete 88%, straight to the door.
- **Yard edge** (n = 29): none 83%, hedge 10%. **Planting:** sparse 50%, continuous 38%, none 12%.
- **Yard trees:** 47%. **Shutters:** 9%. **Walls:** brick 78%. **Trim:** white 72%, cream 16%. **Doors:** black 38%,
  white 31%.

## What the grammar adds that the photos don't settle (ours)

The lattice or brick skirt under a porch (half and half), handrails on porch steps (a third, with stoops more
often), the accent cones that flank some steps, and the late-November colours of the yard trees.
