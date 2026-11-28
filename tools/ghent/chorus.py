"""
chorus.py: what Ghent's people say when you eavesdrop. Not invented voices: the argument over the proposal as the
campaign's own pages and the City's plan put it, every line quoted word for word from its source, with the page.

  - Policy Brief No. 1, Section IX ("Anticipated Objections and Responses"): five objections, each answered by its
    own response (a group answers an objection with the brief's reply to it);
  - the zines' neighbourly questions, answered by the zines;
  - Brief 1 on the money, including what it says it doesn't know;
  - NFK2050, the comprehensive plan, including where it is cooler on the proposal than the maximal reading here
    (it prefers arterials and collectors to neighborhood interiors, p. 69).

A line's value sets the speaker's mood in the walk (public/sparrow/chorus.js): an objection is a gripe, an answer a
love, a caution a shrug-ish meh. `near` names the places it's likelier to come up (build.py fills them from the
proposal). `lead` is ours, said before a quotation so it's heard as one.

Run:  tools/vibe-render/.venv/bin/python tools/ghent/chorus.py --verify ../jason-edelman.org
      (checks every quotation against the campaign's pages in a local checkout; build.py writes chorus.json)
"""

import html
import os
import re
import sys

SITE = "https://jason-edelman.org/ghent-streets/"
BRIEF1 = ("Ghent Streets, Policy Brief No. 1", SITE + "policy/brief-01.html", "ghent-streets/policy/brief-01.html")
BRIEF2 = ("Ghent Streets, Policy Brief No. 2", SITE + "policy/brief-02.html", "ghent-streets/policy/brief-02.html")
ZINE1 = ("Ghent Streets, Zine No. 1", SITE + "zines/zine-01.html", "ghent-streets/zines/zine-01.html")
ZINE2 = ("Ghent Streets, Zine No. 2", SITE + "zines/zine-02.html", "ghent-streets/zines/zine-02.html")
NFK2050 = "https://www.norfolk.gov/DocumentCenter/View/108574"

OPINIONS = [
    # Brief 1, §IX: the objections, and the brief's own answers.
    {"name": "objection: character", "doc": BRIEF1, "where": "§IX", "value": 0.2, "near": "converted_houses",
     "says": "This will change the residential character of the neighborhood.", "answer": "answer: character"},
    {"name": "answer: character", "doc": BRIEF1, "where": "§IX", "value": 0.85, "near": "converted_houses",
     "says": "The physical character of the district, which the HC designation exists to protect, is unchanged. "
             "The use of a building can change without any exterior modification."},
    {"name": "objection: demand", "doc": BRIEF1, "where": "§IX", "value": 0.2, "near": "converted",
     "says": "There isn't demand.", "answer": "answer: demand"},
    {"name": "answer: demand", "doc": BRIEF1, "where": "§IX", "value": 0.85, "near": "converted",
     "says": "The displacement is visible on the ground: Colley Avenue, Granby Street, and the HC-G1 corridor absorb the "
             "commercial demand that HC-G2 cannot legally serve."},
    {"name": "objection: review", "doc": BRIEF1, "where": "§IX", "value": 0.25, "near": "food",
     "says": "These uses should get case-by-case review.", "answer": "answer: review"},
    {"name": "answer: review", "doc": BRIEF1, "where": "§IX", "value": 0.8, "near": "food",
     "says": "Small, ground-floor neighborhood retail does not need one: the impacts are well understood, the precedent is "
             "extensive (every PCO corridor in Norfolk), and the performance standards proposed here set size, hours, "
             "signage, outdoor seating, parking and delivery rules directly."},
    {"name": "objection: overlay", "doc": BRIEF1, "where": "§IX", "value": 0.3, "near": "colley",
     "says": "This is better handled as a PCO overlay.", "answer": "answer: overlay"},
    {"name": "answer: overlay", "doc": BRIEF1, "where": "§IX", "value": 0.8, "near": "colley",
     "says": "The difference is scope: a PCO would apply only to designated streets; this amendment applies HC-G2-wide, "
             "treating the entire district as the walkable urban neighborhood it is rather than carving out selected "
             "corridors for different treatment."},
    {"name": "objection: nobody asked", "doc": BRIEF1, "where": "§IX", "value": 0.2, "near": None,
     "says": "If there were demand, someone would have asked by now.", "answer": "answer: nobody asked"},
    {"name": "answer: nobody asked", "doc": BRIEF1, "where": "§IX", "value": 0.85, "near": None,
     "says": "There is nothing to ask for. These uses are prohibited in HC-G2, so an operator has no application to file, "
             "and the absence of applications measures the rule, not the demand."},
    # The zines: a neighbour's question, and the zine's answer.
    {"name": "question: historic protection", "doc": ZINE1, "where": "", "value": 0.3, "near": "hcg2",
     "says": "But isn't the historic protection the whole point?", "answer": "answer: historic protection"},
    {"name": "answer: historic protection", "doc": ZINE1, "where": "", "value": 0.85, "near": "hcg2",
     "says": "A coffee shop in an existing ground-floor space doesn't change any of that. The building and the street look "
             "exactly the same."},
    {"name": "question: Colley", "doc": ZINE2, "where": "", "value": 0.3, "near": "colley",
     "says": "But didn't the PCO change Colley's character?", "answer": "answer: Colley"},
    {"name": "answer: Colley", "doc": ZINE2, "where": "", "value": 0.85, "near": "colley",
     "says": "Drive down Colley and look at it. The Craftsman bungalows behind the storefronts are still there, and so is "
             "the tree canopy."},
    {"name": "question: safety", "doc": ZINE1, "where": "", "value": 0.35, "near": "corners",
     "says": "Are active streets actually safer? What does the evidence say?", "answer": "answer: safety"},
    {"name": "answer: safety", "doc": ZINE1, "where": "", "value": 0.8, "near": "corners",
     "says": "Urban planners have argued for decades that streets with active commercial frontage — cafés, shops, offices "
             "with regular foot traffic — are safer for everyone on them."},
    # Brief 1 on the money, what it knows and what it doesn't.
    {"name": "the taxes", "doc": BRIEF1, "where": "§II", "value": 0.8, "near": "converted",
     "says": "Ground-floor businesses pay meals, sales and business license taxes that houses don't."},
    {"name": "home values", "doc": BRIEF1, "where": "§II", "value": 0.5, "near": None,
     "says": "Whether they also raise nearby home values is less clear; the research is mixed"},
    {"name": "ground floors only", "doc": BRIEF1, "where": "§VI", "value": 0.8, "near": "converted_flats",
     "says": "Commercial uses are permitted on ground floors only. Upper floors of any structure shall remain residential "
             "or institutional."},
    # NFK2050, quoted as Brief 1 quotes it (§VII), with the plan's pages.
    {"name": "the plan: local businesses", "doc": ("NFK2050, p. 92", NFK2050, BRIEF1[2]), "where": "", "value": 0.85, "near": None,
     "lead": "The city's own plan says to", "says": "Preserve local businesses and support new, naturally occurring local "
     "businesses that serve the community"},
    {"name": "the plan: intersections", "doc": ("NFK2050, p. 93", NFK2050, BRIEF1[2]), "where": "", "value": 0.8, "near": "corners",
     "lead": "The plan says it outright:", "says": "Small-scale retail or other complementary community-facing uses are "
     "encouraged at key nodes or intersections, with consistent street frontage"},
    {"name": "the plan: interiors", "doc": ("NFK2050, p. 69", NFK2050, BRIEF1[2]), "where": "", "value": 0.45, "near": "beyond_plan",
     "lead": "The plan also says", "says": "The Secondary, non-residential uses envisioned for the Sustain "
     "areas are appropriate primarily along arterials and collector streets, preserving the residential character of the "
     "neighborhood interiors."},
]

# Quoted elsewhere in the walk (the "Day to day" panel), and checked the same way.
CITED = [
    {"name": "pace", "doc": BRIEF2, "says": "a steady state of 15–40 in years 6–10"},
]

QUESTIONS = [
    {"question": "You live in one of the most beautiful, walkable neighborhoods in Virginia. So why can't you walk to get a "
                 "cup of coffee?", "doc": ZINE1,
     "why": "Ghent was built in the 1890s as one of Norfolk's first planned neighborhoods. The people who designed it "
            "assumed you would walk."},
]


def build(sets):
    """The chorus file (public/sparrow/chorus.js's format), with each line's places from `sets`."""
    ops = []
    for o in OPINIONS:
        title, url, _ = o["doc"]
        cite = f"{title}{', ' + o['where'] if o['where'] else ''}"
        op = {"name": o["name"], "family": "campaign" if "Ghent Streets" in title else "plan", "source": cite, "url": url,
              "says": o["says"], "value": o["value"], "cite": cite, "max": 300, "features": sorted(sets.get(o["near"], ())) if o["near"] else []}
        for k in ("answer", "lead"):
            if o.get(k):
                op[k] = o[k]
        if o["says"].rstrip().endswith("?"):
            op["opener"] = ""  # a question asks itself
        ops.append(op)
    return {"version": 1, "site": "ghent-streets",
            "note": "Every line is quoted from its source (Ghent Streets' briefs and zines, and NFK2050 as Brief 1 quotes it); "
                    "who says it is ours.",
            # How people start and answer (ours): plain, neighbourly, never a quotation.
            "voice": {"openers": {"gripe": ["Look,", "Here's my worry.", "I'll say it:", "My problem is this."],
                                  "meh": ["To be fair,", "Mm.", "Then again,"],
                                  "love": ["The way I see it,", "Honestly?", "Think about it.", "It's simple."],
                                  "shrug": ["Hm.", "Who knows."]},
                      "replies": {"agree": ["And", "Right, and", "Plus,"], "differ": ["But", "Sure, but", "Come on, though.", "Hang on."],
                                  "shrug": ["Couldn't tell you."]}},
            "opinions": ops,
            "questions": [{"question": q["question"], "why": q["why"], "source": q["doc"][0]} for q in QUESTIONS]}


def verify(checkout):
    """Every quotation, word for word, in the campaign's pages."""
    def text(rel):
        s = open(os.path.join(checkout, rel)).read()
        s = re.sub(r"<(script|style)[^>]*>.*?</\1>", "", s, flags=re.S)
        return re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", " ", s)))
    cache, bad = {}, []
    norm = lambda t: t.replace("’", "'").replace("“", '"').replace("”", '"')
    for item in OPINIONS + QUESTIONS + CITED:
        rel = item["doc"][2]
        page = cache.setdefault(rel, norm(text(rel)))
        for key in ("says", "question", "why"):
            if key in item and norm(item[key]) not in page:
                bad.append((rel, item[key][:70]))
    for rel, q in bad:
        print("NOT FOUND", rel, "|", q)
    n = len(OPINIONS) + len(QUESTIONS) + len(CITED)
    print(f"{n - len(bad)} of {n} quotations found verbatim")
    return not bad


if __name__ == "__main__":
    if "--verify" in sys.argv:
        sys.exit(0 if verify(sys.argv[sys.argv.index("--verify") + 1]) else 1)
