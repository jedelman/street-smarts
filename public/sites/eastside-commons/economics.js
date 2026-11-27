// The plan's economics, shared by the walk's Money panel (sites/eastside-commons/money.js) and the long read
// (read.html, #adds-up): every sourced figure and stated assumption in one place, and the
// whole-plan arithmetic on the plan's measurements (tools/vibe-render/plan_numbers.py). The
// sources are listed in read.html's "How the numbers are built".
import { FLOW } from "../../sparrow/flows.js";

export const SF = 10.7639; // square feet in a square metre

export const A = {
  usable: FLOW.usable, homeSf: FLOW.homeSf, perHome: 2.27, spendPerHome: FLOW.spendPerHome, // ours; Census; BLS
  m2PerDesk: FLOW.m2PerDesk, deskWeek: FLOW.spendPerWorkweek, deskWeeks: FLOW.workweeks,   // HCA 2015; ICSC 2012; ours
  paris: 28, barcelona: 37,                                            // per 1,000 residents: Apur 2023; Barcelona 2024
  bcnShops: 21167, bcnServices: 33740, bcnAll: 61875,                  // Barcelona 2024 commercial census
  buildPsf: (205 + 290) / 2, runPsf: 10.33,                            // RLB Q3 2026 (Charlotte strip retail, hard); IREM 2024
  softShare: 0.30, landPaid: 11e6 + 2.4e6,                             // ours (BAE 2025, Terner 2023, Brookings 2020); Norfolk EDA, 2020
  oldAsk: 14.75, newAsk: 35,                                           // Thalhimer Q3 2026 (Norfolk, NNN); ODU 2026 (new construction, NNN)
  // The whole plan's rents, for the public-finance lead.
  aptPsfMo: 1.80, aptVac: 0.05, aptOpex: 0.28,                         // C&W | Thalhimer Q3 2026 (Norfolk, effective); BAE 2025; BAE 2025
  officeRent: 26.78, officeVac: 0.10, officeOpex: 0.32,                // C&W | Thalhimer Q3 2026 (Norfolk Class A, full service); ours (Norfolk 14.1%); BAE 2025
  shopNet: 27, shopVac: 0.05,                                          // a developer's return (above); ours
  gpdPerson: 100, gpdDesk: 15,                                       // 9VAC25-790-460 Table 3: dwellings per person; factories' low end (offices aren't listed)
  swiftWellGpd: 2e6,                                                 // HRSD, James River SWIFT: each well recharges up to 2 million gallons a day
  seniorRate: 0.0618, cityRate: 0.0411, stackYears: 30,                // Virginia Housing, St. Paul's TWG B2 (2025); Norfolk GO 2025A; ours
  stPauls: [                                                           // St. Paul's TWG B2, 4% application, 6/25/2025: $50,939,077
    ["Tax-credit equity", "U.S. Bancorp, $0.81 a credit", 15988355],
    ["State and local loans", "REACH at 3.95%, Virginia Housing Match, two state housing funds", 7954154 + 3000000 + 1400000 + 2000000],
    ["First mortgage", "Virginia Housing, at 6.18%", 11772000],
    ["HUD Choice Neighborhoods grant", "three awards, 2018 to 2024", 4796999],
    ["Norfolk's housing authority", "public housing funds", 1950000],
    ["The developer's deferred fee", "paid later, from the rents", 1666963],
  ],
  homesBuildPsf: (220 + 375) / 2, taxRate: 1.23 / 100,                 // RLB Q3 2026 (Charlotte multifamily, hard); City of Norfolk
  officeBuildPsf: (265 + 370) / 2,                                     // RLB Q3 2026 (Charlotte secondary offices, hard)
  walkup: [31.8, 42.0, 26.2], downtown: [15.9, 69.7, 14.5],            // C&W / Places Platform, Reimagining Cities, p. 75 and summary
  shopSf: 900, livingWage: 47273, margin: 0.311,                       // ours; MIT; Census ARTS 2022
  granby: 55e6,                                                        // Downtown Norfolk Council, FY2023
  capital: {
    developer: { label: "A developer", note: "a 7.08% cap rate + 1.25 points, on the whole cost", yield: 0.0708 + 0.0125 }, // Northmarq Q3 2026; Terner 2023, BAE 2025
    bank: { label: "A bank loan", note: "prime rate, 7.00%, 25 years", rate: 0.07, years: 25 },
    public: { label: "Public money", note: "Norfolk's own bonds, 4.11%, 30 years", rate: 0.0411, years: 30 }, // Norfolk GO 2025A, 2045 maturity
    paid: { label: "Already paid for", note: "running costs only", rate: 0, years: 0 },
  },
};

/** Yearly payment that pays off 1 over `y` years at rate `r` (1/y at no interest). */
export function ann(r, y) { return r ? r / (1 - Math.pow(1 + r, -y)) : 1 / y; }

/** A year's payments on 1 borrowed, the city's share at its bond rate, the rest as a first mortgage. */
export function borrowFactor(city) { return city * ann(A.cityRate, A.stackYears) + (1 - city) * ann(A.seniorRate, A.stackYears); }

/**
 * The whole plan, from plan_numbers.py's measurements `n`: floor areas, what it costs, what it
 * earns at today's rents, the rate that pays it off, the grants and credits it needs, homes,
 * people, desks, what they spend nearby, and its property tax.
 */
export function whole(n) {
  const floorSf = n.floor_area_m2 * SF, upperSf = n.upper_floor_area_m2 * SF;
  const liveSf = (n.live_m2 != null ? n.live_m2 : n.upper_floor_area_m2) * SF, workSf = (n.work_m2 || 0) * SF, playSf = floorSf - liveSf - workSf;
  const cost = { homes: liveSf * A.homesBuildPsf * (1 + A.softShare), work: workSf * A.officeBuildPsf * (1 + A.softShare), shops: playSf * A.buildPsf * (1 + A.softShare) };
  cost.total = cost.homes + cost.work + cost.shops + A.landPaid;
  const noi = {
    homes: liveSf * A.aptPsfMo * 12 * (1 - A.aptVac) * (1 - A.aptOpex),
    work: workSf * A.officeRent * (1 - A.officeVac) * (1 - A.officeOpex),
    shops: n.shop_fronts * A.shopSf * A.shopNet * (1 - A.shopVac),
  };
  noi.total = noi.homes + noi.work + noi.shops;
  let lo = 0, hi = 0.2;
  for (let it = 0; it < 60; it++) { const mid = (lo + hi) / 2; if (ann(mid, A.stackYears) * cost.total > noi.total) hi = mid; else lo = mid; }
  const homes = liveSf * A.usable / A.homeSf, desks = (n.work_m2 || 0) * A.usable / A.m2PerDesk;
  return {
    floorSf, upperSf, liveSf, workSf, playSf, cost, noi, payRate: lo,
    freeNeed: Math.max(0, 1 - noi.total / (borrowFactor(0.35) * cost.total)),
    homes, people: homes * A.perHome, desks,
    spend: homes * A.spendPerHome, deskSpend: desks * A.deskWeek * A.deskWeeks,
    sewageGpd: homes * A.perHome * A.gpdPerson + desks * A.gpdDesk, // wastewater, gallons a day, at design flows
    tax: (playSf * A.buildPsf + liveSf * A.homesBuildPsf + workSf * A.officeBuildPsf) * A.taxRate,
  };
}
