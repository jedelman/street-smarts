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
   that are genuinely transport-agnostic. As of §3.2–§3.3's current
   revision, "reuse" means reusing atproto *as atproto actually is* —
   individually-keyed repos, no group-owned signing entity — rather than
   an earlier draft's group-DID extension to it.
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

### 3.2 Identity: individual, not collective — a correction to earlier
drafts

`did:plc` is still out — it resolves through a public, semi-centralized
directory, which leaks exactly the existence-and-activity signal this
protocol is trying to avoid. `did:web` is still out for the same
collectives that can't run a public server. Both conclusions from earlier
drafts survive. What doesn't survive: earlier drafts derived one
`did:iroh` *per cooperative*, then spent most of §3.7–§3.8 solving how a
group could safely hold that single identity's key together (threshold
signatures, distributed key generation, rekey-as-succession). That whole
problem dissolves once identity is individual: **every member has their
own ordinary `did:iroh:<nodeid>`, one Ed25519 keypair, exactly the key
they'd need for any other iroh use — no group key, no DKG, no threshold
scheme, because there is no group secret for any of that machinery to
protect.** "Eleanor's" is not a DID at all; see §3.3.

Resolution still works the way earlier drafts intended: `did:iroh:<nodeid>`
resolves by dialing the node and asking for its own signed DID document,
which only works if you already have a route to it — identity and
reachability stay collapsed into the same trust boundary, unchanged by
this revision.

### 3.3 Data: each member has an ordinary repo; a node is a namespace, not
a repo

Each member runs (or has hosted on their behalf — see §3.7) one ordinary,
single-key-signed MST repo — the standard atproto shape, nothing new,
nothing collectively owned. Records use lexicon collections,
`network.essmesh.node.profile` and `network.essmesh.node.event`, drafted
in full at `lexicons/network/essmesh/node/` — real schema, not a
placeholder, though not yet validated against live atproto tooling (see
`lexicons/README.md`) and still needing input from an actual cooperative
before any field list should be treated as final. This is where the SCED
weighting scheme from `sbci`'s brief (coop_housing_clt=1.0,
worker_coop=0.8, ...) lives as a first-class, self-asserted field —
`profile.category`'s allowed values are copied verbatim from
`tools/sbci/sbci/ess_source.py`'s `CATEGORY_WEIGHTS` keys, the one place
protocol and pipeline share vocabulary directly.

**"Eleanor's" is a namespace, not an entity with its own signing key.** A
topic — identified by its own ID, not by anyone's DID — aggregates
whichever members' relevant records have been included (§3.4 covers the
propagation mechanism). A reader who's synced that topic receives the
union of included members' individually-signed records. There is no
single canonical
"Eleanor's says X" statement unless the group chooses to converge on one
by their own convention (e.g., "the current profile record is whichever
one carries co-signatures from N current members" — itself an ordinary
governance action per §3.7, not something the protocol requires). This is
closer to how real federated systems already work — individually
attributed statements, trust aggregation left to the reader or to
explicit convention — than earlier drafts' single-voice model was.

### 3.4 Transport: two edge types are the entire membership structure;
direct peering does both aggregation and updates

No separate "membership" object exists anywhere in this design. Two
directed edge types over the same set of individual `did:iroh` identity
nodes:

- **`read(namespace → peer)`**: peer is authorized to receive the
  namespace's records.
- **`write(peer → namespace)`**: peer's own records get included when
  others receive the namespace.

"Is X a member of Eleanor's" has no answer beyond "does X currently hold
a `write` edge into Eleanor's namespace" — directly, mechanically
checkable, not a separate roster that could drift out of sync with the
actual capability grants (§6 covers whether the underlying mechanism
exposes this enumerably).

**Correction to the previous revision, which read "peering for
aggregation and updates" as gossip and overcorrected.** It isn't gossip —
gossip specifically means epidemic, multi-hop relay (a peer forwards what
it received to *its own* peers), and that really would have needed the
strict relay-scoping the previous revision spent a full paragraph
defending. What was actually meant is simpler and doesn't have that
problem: **direct peer-to-peer connections between exactly the edge
holders themselves**, no relay, no intermediary. Two peers who each hold
an edge into the same namespace connect directly and sync — the same
operation whether it's catching up on history or picking up something
published five minutes ago. One mechanism, not the two-mechanism
gossip/backfill split the previous revision invented to solve a
multi-hop leak risk that direct peering never had in the first place.

That also walks back last revision's objection to `iroh-docs`' own
namespace-document model, which was aimed at the wrong target. The
concern was "a converged document implies one canonical merged answer
across every author," which would contradict §3.3's no-single-voice
stance. But `iroh-docs`' actual replication model resolves convergence
*per key*, not across authors — if each member writes under their own
keyspace (e.g. `<member-did>/profile`, `<member-did>/event/<tid>`),
convergence only ever answers "what's the current version of *this
member's own* record," which is exactly wanted (no ambiguity about which
version of someone's own claim is current) and never forces any
resolution *between* different members' claims, which stays exactly as
side-by-side and individually attributed as §3.3 already described. **One
`iroh-docs` document per namespace, synced directly between edge holders,
is the mechanism** — simpler than the previous revision's gossip/backfill
split, and the objection to it doesn't survive close reading of what
`iroh-docs` actually converges on.

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

### 3.7 Governance: who can grant an edge — the only place a quorum still
earns its keep

A correction to earlier drafts, which spent most of this section on FROST
threshold signatures over a group root key (§3.2's earlier version).
There is no root key anymore, so most of that machinery — DKG, threshold
signing ceremonies, rekey-as-succession — is gone, not patched. What
survives, in simplified form, and what's genuinely new:

**3.7.1 The core inversion survives unchanged: capabilities expire by
default; they are not revoked by exception.** Every granted edge — read
or write — is short-lived and self-expiring (hours to a few days, not
months); staying included requires active re-affirmation, not the absence
of a revocation. Silence is loss of access, not retention of it. This
principle didn't depend on FROST to begin with and fits even better
without it: renewal is now just "sign an ordinary message with your own
key," not a multi-round ceremony.

**3.7.2 A real fork in the design, named rather than silently resolved.**
Ordinary data sharing — granting someone a read or write edge into a
specific namespace — needs no group decision at all under a pure
edge-tracking model: whoever already holds a capability can extend one,
the same way inviting someone into a Signal group usually just takes one
existing member choosing to. That's the fully flat reading of "just track
edges." But one action is different in kind from ordinary sharing:
**admitting a new co-signer with standing to grant further edges on the
namespace's behalf** — because that's the action that determines who
gets to keep making this decision, recursively. Two consistent minimal
designs, not resolved here in favor of one without saying so:

- **(a) Fully flat.** Any current write-edge holder can grant any
  capability, including grant-capable status, to anyone. Maximally
  simple, truest to "all we needed to do was track edges." Real cost: a
  single member — compromised, coerced, or just careless — can quietly
  reshape who has standing, by inviting allies who then hold equal
  authority. That's the single-point-of-capture problem this whole
  document exists to avoid, relocated from "who holds the root key" to
  "who can invite new co-signers," not eliminated.
- **(b) Recommended: flat for data, N-of-M for standing.** Ordinary read
  and write access to data stays unilateral, grantable by any current
  holder. Admitting or removing a *governance-eligible* co-signer (someone
  whose grant counts toward future N-of-M decisions) requires N
  individually-signed `consent` `Signal`s (§3.9) from current
  governance-eligible members, referencing the same `Proposal`. Not
  FROST — literally counting valid, distinct individual signatures, which
  any reader can verify without threshold cryptography of any kind.

(b) is the recommendation, on the same reasoning §3.7's earlier drafts
used for the root key: a keyholder class distinct from the membership,
even an accidentally-emergent one under (a), quietly recreates the
hierarchy this document exists to avoid. But it's a real design choice
with a real cost (an additional conceptual layer — "governance-eligible"
— that pure edge-tracking didn't originally need), not a fact the graph
model hands over for free, and it should be stated as a choice if this
gets built, not assumed.

**3.7.3 Tiered friction, simplified — no threshold quorum, just counted
consent:**

| Action | Suggested N-of-M | Rationale |
|---|---|---|
| Grant ordinary read/write access to a specific peer | none — unilateral by any current holder | Zero-friction sharing was the entire point; gating this defeats §3.6's "ticket-passing as easy as the relationships that carry it." |
| Admit a new governance-eligible co-signer | low-medium (e.g. 2–3 of current co-signers) | The recursive, standing-conferring action from §3.7.2 — deserves real but not maximal friction. |
| Remove a governance-eligible co-signer's standing | high (e.g. most of current co-signers) | Consequential and adversarial; should require the group to actually deliberate, same as expelling a member would in the underlying human organization. |
| Change the N-of-M policy itself | unanimous among current co-signers | The constitution-amendment case — see §3.8. If a subset could lower their own oversight threshold unilaterally, every other row is decorative. |

**3.7.4 Transparency: governance events are records, not administrative
side effects.** Unchanged from earlier drafts in substance, mechanism
updated per §3.4: every grant, renewal, and `consent`/`block`/`exit`
`Signal` is a signed record in a `governance` collection within the
namespace's document, synced directly to every current holder the same
way any other record is — transparency as a property of the topology
(reaching everyone with a live connection as it happens, no separate
broadcast step), not a log someone has to remember to check. A member who
was offline catches up on reconnect via the same direct sync against
another edge holder, same as any other missed history — one mechanism,
not a live/backfill split.

**3.7.5 Portability was never about a group key, and is cleaner now.**
Each member's own repo (§3.3) is theirs regardless of what happens to any
shared namespace — there was never a group key for a hostile operator or
majority to hold hostage in the first place. Losing access to a shared
namespace is a data-*inclusion* problem (your records stop being synced
into that aggregation), never a data-loss event: nothing about your own
repo depends on anyone else's cooperation.

**3.7.6 Honest costs, rewritten.** N-of-M consent-by-counting reveals
exactly who signed, always — unlike FROST's aggregate output, which
didn't distinguish which *t* of *n* participated. For a `block` or a
contested co-signer decision, that's a real, narrow opsec cost: outsiders
or other members can see precisely who did and didn't consent, which
could matter for someone's safety if patterns get correlated over time.
Worth the group deciding that's acceptable, not assuming it away. Second
cost: "governance-eligible" as a status distinct from "has a write edge"
is a real conceptual addition this document is choosing to make (§3.7.2),
not something free. None of §3.7 has been validated against `iroh-docs`'s
actual current API surface (§3.4) — in particular, whether it exposes an
enumerable list of current
capability/topic holders at all, which the governance-eligible-roster
idea depends on (§6).

### 3.8 Fission and constituent power, without a group key left to fight
over

Earlier drafts located this document's Hardt-and-Negri moment in
unanimous *rekey* — the act of re-founding a threshold-held group
identity. That artifact (a single group `did:iroh`) no longer exists, and
the insight is sharper without it, not weaker: **there was never a
constituted-power object — a group DID — to seize, defect from, or need
re-founding.** Continuing together is an ongoing, individually-signed
cooperative act that never crystallizes into a single sovereign identity
in the first place. Nothing to capture because nothing was ever
centralized to begin with; the multitude staying plural isn't an escape
hatch built into the protocol anymore, it's just what the protocol *is*.

The closest remaining analog to a constituent act is §3.7.3's "change the
N-of-M policy itself" and "admit/remove a governance-eligible co-signer"
rows — still the place the group is deciding who it is, still
meaningfully higher-friction than ordinary sharing, but carrying no
cryptographic ceremony. And fission is, if anything, easier than earlier
drafts described: a subset that can't reach the N-of-M for a governance
change doesn't need §3.7.5's portability guarantee to escape a shared
identity, because per §3.3 there was never a shared identity to escape —
they just spin up their own namespace, populate it from whichever
individually-signed records (their own, already portable by construction)
they want to carry forward, and grant edges to whoever they choose. No
succession record, no redirect, no "official name" to contest, because
there is no name attached to the group at all — only to individuals and
to namespaces, and a new namespace needs nobody's permission to exist.

The `exit` `Signal` (§3.9) still earns its keep here, in simplified form:
a governance-eligible member's own self-signed withdrawal, shrinking the
effective *M* for future N-of-M governance decisions — the same
voluntary-departure-vs-hostile-holdout distinction as before, just
operating on a counted roster instead of a threshold-share set.

### 3.9 Vote primitives: minimal cryptography, everything else is convention

The mistake to actively avoid: encoding a specific decision-making
procedure — majority vote, consensus-minus-one-block, Robert's Rules —
into the protocol. A cooperative that already knows how to run a hard
meeting doesn't need software telling it how to deliberate; it needs
software that can't be argued with about whether quorum was actually met.
`Proposal` and `Signal` are drafted in full as lexicons at
`lexicons/network/essmesh/governance/` — there is deliberately no
`Ratification` lexicon; see `lexicons/README.md` for why giving it a
record type of its own would quietly reintroduce the aggregation step
this design spent several revisions removing. Split accordingly, with a
hard boundary between the two layers:

**Social layer — expressive, human, cryptographically inert.** A `Signal`
record: any member can publish one, at any time, attached to a
`Proposal`. Type is one of `consent | stand_aside | block | abstain |
exit`, plus free text — that vocabulary because it's what a
consensus-trained group already uses, and collapsing it to `yes/no` would
be a regression, not a simplification. `exit` is the odd one out and the
only type with a defined mechanical effect rather than being purely
informational: a governance-eligible member's own signed withdrawal of
their claim to future participation, which — per §3.8's fission
discussion — shrinks the effective *M* for future N-of-M governance
decisions (§3.7.3) rather than freezing them. Every `Signal` type is an
ordinary signed record from the member's own individual key — never a
threshold operation, no special status, no group key involved anywhere in
this design. This is where discussion, "I'll go along but want my concern
noted," and everything else genuinely human-shaped lives, exactly as
messy as a real meeting, because the protocol doesn't touch it.

**Ratification layer — mechanical, minimal, the only thing with actual
teeth.** A `Ratification` is nothing but N distinct, valid individual
`consent` `Signal`s over the same `Proposal`, counted against whatever N
its class requires (§3.7.3's table) — not aggregated, not threshold-signed,
just counted by any reader capable of checking N ordinary signatures. It
either meets that count or it doesn't exist. No cryptographic
representation of "no" is needed: a `consent` `Signal` already is the
only "yes" that has power, and declining to sign is every other outcome
at once.

**What a `block` Signal actually does, and doesn't do.** At low-N tiers
(admitting a new co-signer, say 2–3 of current co-signers) a block cannot
stop willing consenters by itself — the rest just proceed. That's not a
gap, it matches real consensus practice: you don't give block power over
routine admission decisions. The tier that already requires unanimity
(§3.7.3's "change the N-of-M policy itself" row) is exactly where a block
is *structurally* sufficient, since unanimous-minus-one can never reach
unanimous. For everything in between, say the honest thing plainly:
**a block is a social fact enforced by client convention, not by
cryptography.** An honest reference client refuses to build, relay, or
act on a `Ratification` whose `Proposal` has an outstanding, un-withdrawn
`block` Signal from an eligible member — the same way a block works in a
real meeting: nothing physically stops the room from acting anyway, the
group's shared practice is what makes it matter. Write that down as a
client norm, and don't dress it up as a cryptographic guarantee it isn't
— that distinction (cryptography for privacy and authentication;
everything about how a decision is actually made is convention, enforced
by the humans and the software they choose to run) is the design
principle this whole section follows, not just this one paragraph.

## 4. Relationship to `tools/sbci/`

If this existed, `sbci/ess_source.py` would gain a mesh-backed loader
next to the GeoJSON-fixture loader it has now — pulling `sced` input from
whichever namespace(s) granted a read edge to the pipeline's own
`did:iroh`, instead of a hand-seeded four-entry fixture with unverified
coordinates. That also closes the loop on the `data_fabric.py` finding: a
real, willing, consent-based source on the commons side, rather than
either "no data" or "scrape it without asking."

## 5. What this document is not

Not a commitment that Eleanor's or any real cooperative wants this, needs
this, or has been asked. Not a claim that iroh's current APIs
(`iroh-docs`, formerly `iroh-sync`) already support everything described
here — they weren't checked against this design in detail and some of
§3.3–3.5 may need real API research before it's buildable. Not a
replacement for actually talking to XES, Pam a Pam, or a
Norfolk cooperative about what they'd want, if anything — this is an
engineer's-eye-view sketch of what's *technically* possible, which is a
different question from what's wanted. And, worth being honest about
given how much of this document has changed shape in conversation: not a
document that arrived at its current architecture on the first pass —
§3.2–§3.9 moved from a group-owned `did:iroh` secured by FROST threshold
signatures to individually-keyed members and a pure edge-graph, because
the group secret the first version was built to protect turned out not
to need existing at all. That revision is recorded in this document's own
git history, not hidden — a design sketch this order of unfinished should
show its work, not just its current conclusion.

## 6. Open questions, ranked by "blocks anything getting built"

1. Has anyone talked to an actual cooperative about whether any of this
   solves a problem they have? Still ranked first, on purpose — everything
   below is unbuildable-usefully without an answer to this one.
2. Does `iroh-docs` expose an enumerable list of who currently holds a
   capability grant into a namespace? §3.7.2's governance-eligible-roster
   idea depends on being able to check who currently holds a grant-capable
   edge, not just on holding one yourself. Needs a real read of the
   crate's current source, not assumed from memory.
3. **Resolved by correcting a misreading, not by new design work**: an
   earlier revision worried about scoping multi-hop gossip relay to
   exactly the edge set, which would have been a real, load-bearing
   constraint. It doesn't apply — §3.4 now describes direct peer-to-peer
   sync between edge holders, not epidemic/relayed gossip, so there's no
   relay boundary to leak beyond in the first place. Kept as a record that
   this was worried about and the worry doesn't survive the correction.
4. §3.7.2's fork — flat capability-granting (a) vs. N-of-M consent
   specifically for governance-eligible status (b) — is a live, unresolved
   design choice, not something this document has picked on the group's
   behalf despite recommending (b). Whoever actually builds this should
   decide it deliberately, ideally with input from a cooperative that
   would use it.
5. N-of-M consent-by-counting (§3.7.6) reveals exactly who signed, every
   time — a real, narrow opsec cost relative to what FROST's aggregate
   signature would have hidden. Is that acceptable, given who this is
   for? Not evaluated here.
6. What does an ordinary member's experience of signing a `consent`
   `Signal` actually feel like in practice — is "sign an ordinary message
   with your existing key" as frictionless as this document assumes, or
   does it still need real UX work to not become its own version of the
   threshold-ceremony problem it was designed to avoid?
7. Hosting-on-behalf-of threat model, now much lighter-weight than earlier
   drafts (§3.7.5) but not zero: what can someone running shared
   infrastructure for a namespace still see or do with an ordinary,
   unilaterally-grantable read/write edge, and is that residual exposure
   acceptable to a group like Eleanor's?
8. **Superseded, not open**: FROST tooling maturity, UCAN-over-a-threshold-DID,
   rekey-as-succession, and DID-identifier-continuity-across-a-rekey were
   all real open questions against the group-DID/FROST architecture in
   earlier drafts. None of them apply to the current edge-graph
   architecture (§3.2–§3.8) — there is no group DID to rekey or need
   continuity for. Kept here as a record that the architecture changed
   underneath them, not because they're still live.
9. New namespace migration: if a group ever wants to move "Eleanor's" to
   a different topic entirely (not fission — the same group, deliberately
   relocating), is there a clean way to signal "this topic supersedes
   that one" to existing edge-holders, or does every holder need to be
   individually re-shared with the new topic by hand? Unlike the old
   DID-redirect problem this replaces, there's no natural place to put
   that signal, since a bare topic ID carries no signature of its own the
   way a DID document did.
10. **Mostly resolved by §3.4's correction, worth confirming against the
    real API rather than fully closing**: new-member historical access
    was a real open question when "live propagation" and "backfill" were
    two separate mechanisms with different semantics. With one mechanism
    (direct sync of the namespace's `iroh-docs` document), the answer
    should just fall out of what document sync means by definition —
    syncing a document gets you its current state, not merely a
    subscription to future changes. What's still unverified: does
    `iroh-docs` actually behave that way in practice, and does a new
    member's first sync need to happen against one specific peer they can
    reach, or can it pull from any current holder — which would make this
    faster in practice than it reads on paper.
11. The four lexicons at `lexicons/` (§3.3, §3.9) are a careful draft
    following documented atproto lexicon conventions, not run through an
    actual lexicon validator or checked against current atproto tooling —
    same caveat as everything else in this document that hasn't touched a
    real API this session. Before anyone builds against them: validate
    the schemas themselves, and get an actual cooperative's eyes on
    `profile`'s and `event`'s field lists specifically, since those are
    the two records asking someone to describe themselves, not just the
    two managing protocol mechanics.
