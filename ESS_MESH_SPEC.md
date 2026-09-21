# ESS Mesh — a capability-scoped, audit-by-construction protocol for cooperative/solidarity-economy nodes

Status: **design sketch, no code**. This document exists to pin down the
architecture before anything gets built, per this repo's own norm (see
`SPEC.md`, `PRIMITIVES_SPEC.md`) of writing the spec first.

## 1. The problem this solves

`tools/sbci/sbci/data_fabric.py` documents a finding from building the
Spatial Bio-Capital Index pipeline: every capital-adjacent data source
tested (OSM, Census, Open Data BCN) is a real bulk-query API; every
commons/ESS source tested (XES, Pam a Pam) is a browse-only directory with
no API. The naive fix — crawl the directories, publish a public queryable
index — trades one problem for a worse one. A cooperative like Eleanor's
(an anarchofeminist book/info shop in Norfolk, referenced here as the
motivating case, not as a subject with confirmed technical requirements —
nobody has asked them anything yet) has real reasons to be careful about
who can query its location and activity pattern. A public, crawlable,
geolocated database of "here are the radical spaces" is a liability to
the people it lists, not a gift to them. See `tools/sbci/README.md`'s
"Finding" section and this repo's own anti-measurement stance in its
top-level README for the fuller argument.

So the actual design target isn't "make ESS data as legible as parcel
data." It's: **searchable, but every search is traced by the node being
searched, and disclosure is the node's own choice, tier by tier.**

## 2. Design goals, in priority order

1. **Zero ambient legibility.** No public index, no relay, no firehose. A
   node that hasn't granted a capability to a peer is invisible to that
   peer, full stop.
2. **Audit-by-construction, not audit-as-a-log.** Because there's no
   intermediary, a query has to physically reach the node being searched.
   The node inherently sees who asked, when, for what. This is a property
   of the topology, not a feature someone could forget to enable or a log
   file someone could fail to check.
3. **Tiered disclosure, node-controlled.** A cooperative should be able to
   publish *something* discoverable (a public storefront: name,
   neighborhood, category) without that implying the full record (address,
   hours, contact, activity data) is public too. Real-world businesses
   already do this — a listed phone number doesn't mean the owner's home
   address is public. The protocol should make that distinction native,
   not bolted on.
4. **Reuse a mature data model.** Don't reinvent "what fields does a
   cooperative-economy record have" or "how do you sign and version a
   repo of records" — atproto's lexicon schema system and Merkle-search-tree
   repo format already solve both, and solve them well. Reuse the parts
   that are genuinely transport-agnostic.
5. **Zero-labor publishing**, from the earlier conversation about node
   needs — whatever a collective has to do to join, it has to be closer to
   "fill out one form" than "run a server."

Goals 1–3 are in real tension with 4–5: atproto's tooling (indigo, the
official SDKs, AppView infra) is built assuming the PDS→relay→firehose
pipeline, which is exactly the public-by-default topology goal 1 rejects.
That tension is the central design problem this document has to resolve,
not paper over.

## 3. Architecture

### 3.1 Split atproto's layers, keep two, replace one

atproto is usually described as one thing, but it's three separable
layers:

| Layer | What it is | Transport-dependent? |
|---|---|---|
| Identity | DIDs — a public key with a resolvable document | Partially — `did:plc` resolves via a public directory; other methods don't |
| Data | Signed, content-addressed Merkle search tree (MST) per repo, versioned by commit | No — it's just a data structure |
| Schema | Lexicons — typed record definitions (JSON Schema-like) | No — pure typing convention |
| Sync/distribution | PDS hosts a repo publicly; relay crawls it; AppViews index the firehose | **Yes — this is the public-by-default part** |

The proposal: **keep the data layer (MST repos) and schema layer
(lexicons), replace the sync layer with iroh, replace the identity
layer's resolution mechanism.**

### 3.2 Identity: a custom DID method off iroh's own keys

`did:plc` is out — it resolves through a public, semi-centralized
directory, which leaks exactly the existence-and-activity signal this
protocol is trying to avoid. `did:web` is out for the same collectives
that can't run a public server.

Proposal: `did:iroh` (name placeholder), where the DID *is* derived
directly from an iroh `NodeId` (already an Ed25519 public key — iroh
nodes are self-certifying by construction). No directory lookup needed to
resolve identity; resolving a `did:iroh:<nodeid>` just means "dial this
node and ask it for its own signed DID document," which only works if
you already have a route to the node — i.e., resolution itself requires
the same capability grant as everything else. Identity and reachability
collapse into the same trust boundary, which is the property we want.

**Key custody, for a cooperative and not an individual**: this is a real
open question, not solved here. A collective needs either a shared key
(bad — no revocation without everyone re-keying) or a small multi-sig /
threshold scheme over the node's signing key (better, more complex to
implement, needs a real answer before this is usable by an actual
cooperative rather than a single maintainer).

### 3.3 Data: one MST repo per node, one lexicon collection

Each participating node runs (or has run on its behalf — see §3.5) a
small repo containing records in a new lexicon collection, tentatively
`network.essmesh.node.profile` and `network.essmesh.node.event`
(governance tier, category, hours, etc. — schema TBD, needs input from an
actual cooperative before it's finalized, not invented wholesale here).
This is where the SCED weighting scheme from `sbci`'s brief
(coop_housing_clt=1.0, worker_coop=0.8, ...) could live as a first-class,
self-asserted field rather than something an outside pipeline guesses at.

### 3.4 Transport: iroh-docs namespaces as capability grants

Instead of a public PDS, each node's repo lives in an `iroh-docs`
namespace. Namespace capabilities (read-only "tickets") are the actual
disclosure mechanism: a node shares a read ticket with a specific peer
(another cooperative, a trusted aggregator, a researcher with standing
permission) out of band — over Signal, in person, however trust is
already established in these networks — and only ticket-holders can sync
the repo. No ticket, no data, no knowledge the node exists on the mesh at
all.

### 3.5 Tiered disclosure: two namespaces, not one

To satisfy goal 3 without contradicting goal 1, a node that wants any
public presence runs (or delegates — see below) **two namespaces**:

- A `public` namespace, openly discoverable, containing only what the
  node explicitly wants a stranger to find (name, category, neighborhood
  — no address, no hours, no activity data). This is the "storefront."
- A `private` namespace, capability-gated, containing the full record.
  Access is granted per-peer, per-ticket, revocable.

A search against the mesh is then two different operations with two
different trust models: browsing the public namespaces (which, note,
*could* be aggregated by a willing index like XES without contradicting
anyone's opsec, since it's opt-in-public by design) vs. querying a
specific node's private namespace, which only works if you already hold
a ticket — and which the node sees happen.

### 3.6 The bootstrap/discovery problem — the real cost of this design

Be honest about this rather than gloss over it: **pure capability-gated
search has a cold-start problem.** If you don't already know which nodes
to ask, you can't ask them, full audit-by-construction or not. Someone
searching "is there a co-op bookshop near me" with zero existing tickets
gets nothing from the private layer, by design. The public namespace
(§3.5) is the answer, but it only covers what nodes chose to make
public — which could, in the limit, be as thin as "we exist, we're a
bookshop, we're in Norfolk," pushing all the interesting data behind a
trust relationship a stranger doesn't have. That's a real tradeoff, not a
bug: it's the same tradeoff every mutual-aid network already makes
informally (you find out about the actually-useful stuff by knowing
someone, not by searching), the protocol just makes it structurally
explicit instead of ad hoc.

### 3.7 Who actually runs the node

Almost nobody in a five-person cooperative wants to run an iroh node and
manage MST commits. Realistic answer: a small number of trusted
operators (maybe street-smarts itself, maybe a sympathetic org) run
hosting infrastructure that holds signing keys *on behalf of* a
cooperative under a delegated-capability model — similar to how a PDS
today hosts a repo on behalf of a user who doesn't run their own server.
This reintroduces a trust dependency (goal 5 vs. goal 1 tension, again)
that needs its own threat model before anyone builds it: what can a
hosting operator see, what can they be compelled to hand over, and does
the cooperative retain the ability to walk away with their own key and
history intact (portability was one of atproto's actual good ideas —
keep it here).

## 4. Relationship to `tools/sbci/`

If this existed, `sbci/ess_source.py` would gain a mesh-backed loader
next to the GeoJSON-fixture loader it has now — pulling `sced` input from
nodes that granted a ticket to the pipeline's own DID, instead of a
hand-seeded four-entry fixture with unverified coordinates. That also
closes the loop on the `data_fabric.py` finding: a real, willing,
consent-based source on the commons side, rather than either "no data" or
"scrape it without asking."

## 5. What this document is not

Not a commitment that Eleanor's or any real cooperative wants this, needs
this, or has been asked. Not a claim that iroh's current APIs
(`iroh-docs` specifically, formerly `iroh-sync`) already support
everything described here — they weren't checked against this design in
detail and some of §3.3–3.5 may need real API research before it's
buildable. Not a replacement for actually talking to XES, Pam a Pam, or a
Norfolk cooperative about what they'd want, if anything — this is an
engineer's-eye-view sketch of what's *technically* possible, which is a
different question from what's wanted.

## 6. Open questions, ranked by "blocks anything getting built"

1. Does `iroh-docs`'s current capability/ticket model actually support
   per-peer revocable read grants the way §3.4 assumes? Needs a real read
   of the current iroh docs/source, not assumed from memory.
2. Threshold/multi-sig key custody for a collective (§3.2) — is there an
   existing scheme worth borrowing, or does this need to be designed from
   scratch?
3. Hosting-on-behalf-of trust model (§3.7) — who would actually run this,
   and what's the threat model for that operator?
4. Has anyone talked to an actual cooperative about whether any of this
   solves a problem they have? (This should probably be answered before
   #1–3, not after.)
