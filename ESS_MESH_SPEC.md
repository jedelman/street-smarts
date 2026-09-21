# Moved: atproto-iroh

The design that lived in this file — a capability-scoped,
audit-by-construction protocol combining atproto's data model with
iroh's transport — has been extracted to its own repository:

**https://github.com/jedelman/atproto-iroh**

The full spec is there as `SPEC.md`, along with the four draft lexicon
schemas that used to live in `lexicons/` here.

## Why it moved

The design turned out to be general-purpose well before this move — see
the spec's own §5. Nothing past its §3.1 actually references
cooperatives, mapping, or `street-smarts`'s own domain (Alexander-pattern
neighborhood analysis) at all; the same substrate would carry private
messaging, offline-tolerant community tools, or any small-group
coordination that needs to work without a server anyone has to trust.
It didn't belong here permanently any more than it belonged permanently
in `tools/sbci/` before that — the motivating case (from `sbci`'s
`data_fabric.py` finding) stays real and worth reading, but the protocol
itself outgrew both homes.

## What's still true here

`tools/sbci/sbci/ess_source.py`'s `CATEGORY_WEIGHTS` is still the
vocabulary `atproto-iroh`'s `node.profile.category` lexicon copies from —
now a cross-repo relationship with no automated check keeping the two in
sync. If you change `CATEGORY_WEIGHTS` here, check whether
`jedelman/atproto-iroh`'s lexicon needs updating too.

This file, and the full design history that led to the extraction, are
preserved in this repo's git log (`git log -- ESS_MESH_SPEC.md` from
before this redirect) if you want the reasoning trail rather than just
the current state.
