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

### 3.6 Discovery rides on relationships that already exist

Correction to an earlier draft of this document, which framed §3.6 as a
"cold-start problem." It isn't one, for this network specifically:
mutual-aid and cooperative networks aren't cold graphs a stranger
searches into — they're warm, already-existing networks of trust (a
worker at Eleanor's knows someone at Coop57 knows someone doing
tenant organizing knows someone at the next info shop). Ticket exchange
over Signal, in person, at an assembly, is not a workaround for a missing
discovery layer — it *is* the discovery layer, the same one these
networks already run on before any protocol existed. The public
namespace (§3.5) still matters for the genuinely cold case (someone with
zero existing relationship to the movement), but it's a courtesy on-ramp,
not the thing the design has to optimize for. Don't build a search engine
for this network. Build the ticket-passing as easy as the relationships
that already carry it.

### 3.7 Delegation: revocable, small-group, transparent by construction

This is the actual hard design problem, correctly identified as harder
than discovery. Almost nobody in a five-person cooperative wants to run
an iroh node and manage MST commits personally, so *someone* ends up
holding infrastructure on the group's behalf — and that's exactly the
point where a protocol can quietly recreate the landlord it was built to
avoid, unless delegation is designed so power can't accumulate or
persist past the group's active consent.

**3.7.1 The core inversion: capabilities expire by default; they are not
revoked by exception.**

The anarchist answer here isn't "the group can revoke the operator if
something goes wrong" — that puts the burden of catching abuse on the
group, after the fact, which is exactly the failure mode of every
"you can always fire the board" institutional safeguard that in practice
never gets invoked until real damage is done. Invert it: every delegation
grant to an operator is **short-lived and self-expiring** (hours to a few
days, not months), and staying operational requires the group to
*actively re-affirm* it on a rolling basis. Silence is loss of power, not
retention of it. An operator who goes rogue, disappears, or gets
compromised loses all capability within one expiry window whether or not
anyone in the group notices in time to act — no revocation ceremony
required for the default case, because there's nothing to revoke; there's
only something to *not renew*.

**3.7.2 Root identity is threshold-held by the group itself, never by the
operator.**

The `did:iroh` root keypair (§3.2) for a cooperative's node should not be
a single key anyone holds, operator included. Split it via a threshold
scheme (FROST over Ed25519 is the live option worth evaluating — see open
questions) so that any *quorum* of the group's own members, not any
single member and never the hosting operator, is required to sign a
delegation grant, a renewal, or a full revoke-and-replace. The operator
only ever holds a *derived, scoped, expiring* capability — never the root
key, never something that outlives the group's active say-so. This is
the piece that makes "revocable" real instead of aspirational: revocation
isn't the group asking the operator nicely to stop, it's the group simply
declining to re-sign, after which the operator's held capability is
cryptographically worthless.

**3.7.3 Tiered quorum: routine operations are cheap, structural changes
are expensive.**

Not every group decision deserves the same friction. A useful split,
borrowed from how sociocratic/consensus groups already grade decisions:

| Action | Suggested quorum | Rationale |
|---|---|---|
| Renew the current operator's capability (routine, every few hours/days) | low (e.g. 2 of 5 active members) | Has to be cheap enough that it actually happens; a threshold so high that renewal itself becomes a burden defeats the "expire by default" safety property by making people ignore it or pre-sign months of renewals to avoid the hassle. |
| Grant a *new* capability scope (e.g., operator gets write access to the private namespace, not just public) | medium (e.g. 3 of 5) | A real change in what the operator can do; deserves more than routine friction. |
| Replace the operator entirely / revoke-and-lock-out | high (e.g. 4 of 5 or unanimous minus absent) | The consequential, adversarial case — should require the group to actually deliberate, same as expelling a member would in the underlying human organization. |
| Change the quorum policy itself | unanimous | The constitution-amendment case. If a subset of the group could lower their own oversight threshold unilaterally, every other safeguard here is decorative. |

**3.7.4 Transparency: delegation events are records, not administrative
side effects.**

Every grant, renewal, and revocation is itself a signed record, written
into the node's own repo (a natural fit for the private namespace, or a
dedicated `governance` namespace visible to all members but not to
outside ticket-holders) the same way any other MST commit is. Because
`iroh-docs` namespaces support live subscription, every member holding a
ticket to that namespace receives the new commit as it happens, not on
next login — this is what makes the transparency *instantaneous* rather
than "available if you go look": nobody has to audit a log, because the
log pushes itself to everyone already watching. A member who was offline
catches up on reconnect via the same sync mechanism (iroh's design target
is exactly this kind of intermittently-connected peer), so the record is
durable, not just a live notification that's lost if you blink.

**3.7.5 What the operator retains regardless.**

Even under a hostile or absent operator, the group must be able to walk
away with their own history intact — this was one of atproto's genuinely
good ideas (account portability) and it applies with more force here,
since the operator here was never trusted with root authority to begin
with. Because the operator only ever held a derived, revocable capability
and the actual repo data syncs to any peer holding a ticket (including
the group's own members' personal devices, if they keep a lightweight
replica), losing the operator is a hosting inconvenience, not a data-loss
event or a hostage situation.

**3.7.6 Honest costs.**

This is real cryptographic engineering, not configuration. FROST
threshold signing has meaningful implementation and UX complexity —
someone in a five-person collective has to actually participate in a
signing ceremony to renew capabilities on a rolling basis, and threshold-
signing ceremonies have a long history of being where usable security
projects die. UCAN-style delegated, attenuable capability tokens (worth
adopting for §3.7.2–3.7.3 rather than inventing a bespoke format) are a
real, existing spec, but nobody has built UCAN-over-iroh or UCAN-issued-
from-a-FROST-threshold-DID before as far as this document's author
checked — that combination needs real prototyping, not just citing the
two specs next to each other. None of §3.7 has been validated against
`iroh-docs`'s actual current API surface.

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

1. Has anyone talked to an actual cooperative about whether any of this
   solves a problem they have? Still ranked first, on purpose — everything
   below is unbuildable-usefully without an answer to this one.
2. Does FROST-over-Ed25519 (or an equivalent threshold scheme) have mature
   enough tooling to threshold-sign both `did:iroh` operations *and*
   MST repo commits, or does the repo signing model assume a single
   keypair in a way that needs its own adapter? (§3.7.2)
3. Does a UCAN-style delegated capability token format map cleanly onto
   "derived scope of a threshold-held DID, handed to an operator's own
   separate keypair," or does the threshold-DID part need a custom
   delegation format instead of reusing UCAN as-is? (§3.7.2–3.7.3)
4. Does `iroh-docs`'s current capability/ticket model support live
   subscription + per-peer revocable read grants the way §3.4 and §3.7.4
   assume? Needs a real read of the current iroh docs/source, not assumed
   from memory.
5. What does the quorum signing ceremony actually feel like for a
   non-technical cooperative member, on a rolling multi-times-a-week
   basis (§3.7.3's routine-renewal row)? This is the UX question that has
   killed most usable threshold-crypto projects historically, and it's
   unanswered here.
6. Hosting-on-behalf-of threat model (§3.7.5) — even with the operator
   holding only a derived, expiring capability rather than root, what can
   a hostile or compelled operator still see or do while a grant is live,
   and is that residual exposure acceptable to a group like Eleanor's?
