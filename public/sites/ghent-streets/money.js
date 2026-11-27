// Ghent Streets' Money panel: adaptive reuse, not construction. Nothing here is built; the question is what
// opening ground floors would bring the city, and what it costs. The walk shows every ground floor that could
// open (the proposal's maximum); the money is Policy Brief No. 2's, which prices 15 to 40 businesses that might.
// Its figures are quoted from the brief, not recomputed here.
const BRIEF2 = "https://jason-edelman.org/ghent-streets/policy/brief-02.html";
const BRIEF1 = "https://jason-edelman.org/ghent-streets/policy/brief-01.html";
const num = (v) => Math.round(v).toLocaleString("en-US");

export function render(el, { numbers: n }) {
  const kpi = (big, small) => `<div class="kpi"><b>${big}</b><span>${small}</span></div>`;
  const opens = n ? n.status.converted : null;
  el.innerHTML = `
    <p class="lede">Adaptive reuse: nothing is built. The question is what opening ground floors would bring the city.</p>
    <div class="kpis">
      ${opens != null ? kpi(num(opens), `ground floors open in this model: every one the proposal reaches that has a building to adapt. ${num(n.converted_in_hcg2)} are in HC-G2, the district the amendment covers.`) : ""}
      ${kpi("$194k&ndash;$488k", "a year in city taxes from 15 to 40 new ground-floor businesses: Policy Brief No.&nbsp;2's scenarios, gross, before spending that would have happened elsewhere in Norfolk anyway.")}
      ${kpi("$22,750", "a year in meals tax from each café or restaurant at $350,000 in sales (6.5%). The meals tax is most of the total.")}
      ${kpi("$227k&ndash;$602k", "with the brief's property-value lines, which it says may not hold: the research on what shops do to nearby home values is mixed.")}
    </div>
    <p>What it costs the city to change the rule, in the brief's words: &ldquo;staff time, legal review, printing.&rdquo; What it costs an owner to open: under the amendment, a café in an existing ground floor needs a zoning certificate, and Architectural Review Board review for any exterior work (<a href="${BRIEF1}" target="_blank" rel="noopener">Policy Brief No.&nbsp;1</a>). Today no permit can allow it.</p>
    ${n ? `<p class="small">In this model: ${num(n.converted_by_kind.food)} cafés and restaurants, ${num(n.converted_by_kind.retail)} shops and ${num(n.converted_by_kind.services)} services, in the brief's stated mix (half food and drink, three tenths retail, a fifth services), each at most 3,500 sq ft. The walk shows the ceiling; the brief prices the 15 to 40 that might open.</p>` : ""}
    <p><a class="more" href="${BRIEF2}" target="_blank" rel="noopener">The whole model, sourced: Policy Brief No.&nbsp;2 &rarr;</a></p>`;
}
