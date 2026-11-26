// What each opinion found, the way a person would say it. One line per
// opinion, written for what that opinion measures, and every number in it is
// one the opinion itself reported: its own details, or its own one-line
// finding. tools/sparrow/chorus.test.mjs checks that no number is anyone
// else's. When an opinion's output doesn't have what its line needs (a new
// opinion, a changed finding), there's no line, and the walk falls back to the
// opinion's own words.
//
// Each entry gets { n(key), m(regex), f(x), says, value }:
//   n("n_plazas")   a number from the opinion's details
//   m(/(\d+) of/)   a match in its one-line finding (throws if there's none)
//   f(106273)       "106,273"

const fmt = (x) => (Math.abs(x) >= 1000 ? Math.round(x).toLocaleString("en-US") : String(x));

const PLAIN = {
  // ---- the geometric chorus (Salingaros)
  levels_of_scale: ({ n, f }) =>
    `Things here come in every size, from ${f(n("smallest_m2"))} square metres up to ${f(n("largest_m2"))}, and mostly in the steps Salingaros likes, two to five times bigger each time.`,
  strong_centers: ({ n }) =>
    `There are ${n("n_centers")} little centres of attention around here, but they're crammed together, ${n("mean_nearest_neighbor_m")} metres apart on average.`,

  // ---- the activist chorus
  ownership_pattern: ({ m }) => {
    const [, known] = m(/\(([\d.]+)% of area\)/);
    const [, untagged] = m(/([\d.]+)% of total area is untagged/);
    return `Everything with an owner on record is held in common. Mind you, that's ${known}% of the land. Nobody's written down who owns the other ${untagged}%.`;
  },

  // ---- Alexander's patterns
  p95_building_complex: ({ m }) => `The buildings come in ${m(/(\d+) P95 complexes/)[1]} clusters instead of lone towers. Some hang together better than others.`,
  p106_positive_outdoor_space: ({ n }) => `All ${n("n_open_space")} outdoor spaces here are proper shapes, not scraps left over between buildings.`,
  p21_four_story_limit: ({ n, m }) =>
    `Almost everything's four stories or less, ${m(/(\d+)% of floor area/)[1]}% of the floor space, and the ${n("n_exceptions_over_4_stories")} taller ones keep their distance from each other.`,
  p127_intimacy_gradient: ({ n, m }) =>
    `Of ${n("n_doors_evaluated")} front doors, only ${m(/(\d+)% open into/)[1]}% open into the most public room. The rest walk you straight into the private bits.`,
  p128_indoor_sunlight: ({ n }) => `Only ${n("n_south_lit")} of the ${n("n_buildings_evaluated")} buildings let the sun into their common room through a south window.`,
  p129_common_areas_at_the_heart: ({ n }) => `Every one of the ${n("n_on_path")} buildings puts its common room where people pass through, not off down a dead end.`,
  p131_the_flow_through_rooms: ({ n }) => `Not one of the ${n("n_buildings_evaluated")} buildings lets you walk a loop through its rooms. Every way through ends in a dead end.`,
  p159_light_on_two_sides: ({ m }) => `Most rooms here get light from two sides. ${m(/(\d+)% of window-bearing wall/)[1]}% of the windows count.`,
  p61_small_public_squares: ({ n }) =>
    `Too many big empty squares. ${n("n_oversized")} of the ${n("n_plazas")} are wider than ${n("max_diameter_m")} metres, and Alexander says past that a square feels deserted.`,
  p108_connected_buildings: ({ n }) => `The buildings that touch are actually joined up. Only ${n("n_apparently_unmerged_touching_pairs")} pairs just stand there pressed together.`,
  p221_natural_doors_and_windows: ({ n }) => `All ${n("n_with_door")} buildings have doors and windows. Every single one.`,
  p130_entrance_room: ({ n }) => `All ${n("n_correct")} buildings have a proper entrance hall, right inside the front door.`,
  p133_staircase_as_a_stage: ({ n }) => `All ${n("n_with_stair")} buildings with an upstairs have a proper staircase in them.`,
  p22_nine_per_cent_parking: ({ m }) => `No parking lots. None. Alexander's limit is ${m(/cap: (\d+)%/)[1]}%, and this uses zero.`,
  p36_degrees_of_publicness: ({ n }) =>
    `Nothing here's busy. ${n("n_in_between")} buildings are in-between, ${n("n_quiet")} are quiet, and not one is a real bustling spot.`,
  p38_row_houses: ({ m }) =>
    `The only building on a pedestrian street is too boxy for row houses: ${m(/mean aspect ratio ([\d.]+)/)[1]} long for every 1 wide, and it wants ${m(/([\d.]+)x long-frontage/)[1]}.`,
  p100_pedestrian_street: ({ n }) =>
    `The ${n("n_pedestrian_streets")} pedestrian streets don't have enough doors on them. Alexander wants ${n("min_entrances_per_100m")} every 100 metres.`,
  p112_entrance_transition: ({ m }) => `Less than half the buildings have a proper in-between space at the front door. ${m(/(\d+)% fall in/)[1]}% do.`,
  p120_paths_and_goals: ({ n, m }) => `Some of these paths don't go anywhere. Only ${m(/(\d+)% have a real goal/)[1]}% of the ${n("n_streets")} lead somewhere at both ends.`,
  p122_building_fronts: ({ m }) => `Most buildings come right up to the path, ${m(/(\d+)% build right up/)[1]}% of them. The rest sit back behind a gap.`,
  p163_outdoor_room: ({ m }) => `The outdoor spaces are only about half enclosed, ${m(/mean enclosure (\d+)%/)[1]}% on average. Not quite rooms.`,
  p192_windows_overlooking_life: ({ n, m, f }) =>
    `Nearly every ground-floor window, ${m(/(\d+)% have real street/)[1]}% of ${f(n("n_windows_checked"))}, looks out on something going on, not a blank wall.`,
  p101_building_thoroughfare: ({ n }) => `All ${n("n_passage_cells")} passages through the buildings actually go through. No dead-end tunnels.`,
  p191_shape_of_indoor_space: ({ n, m, f }) => `The rooms are all odd shapes. Only ${m(/(\d+)% have real area/)[1]}% of the ${f(n("n_interior_cells"))} are anywhere near rectangular.`,
  p114_hierarchy_of_open_space: ({ m }) => `Only ${m(/(\d+)% \(by area\)/)[1]}% of the open space has a bigger space nearby to look out onto.`,
  p115_courtyards_which_live: ({ n }) => `The courtyards are alive. All ${n("n_courtyard_buildings")} have doors opening onto them and a view out.`,
  p165_opening_to_the_street: ({ m }) => `The ground floors are open to the street, ${m(/opening coverage (\d+)%/)[1]}% windows and doors on average.`,
  p102_family_of_entrances: ({ m }) =>
    `The front doors are a bit all over the place. ${m(/(\d+)% cluster within/)[1]}% are near another one, and only ${m(/(\d+)% have a door width/)[1]}% are about the same size.`,
  p110_main_entrance: () => `The main doors are close to the street, but they don't always stand out.`,
  p30_activity_nodes: ({ n, m }) => `The squares aren't where the paths meet. Only ${m(/(\d+)% sit where/)[1]}% of the ${n("n_plazas")} are at a crossroads.`,
  p49_looped_local_roads: ({ m }) => `The little local roads loop around, and they're the right width, ${m(/(\d+-\d+)ft width/)[1].replace("-", " to ")} feet.`,
  p50_t_junctions: ({ n }) => `All ${n("n_three_way")} crossings are T-junctions, which is good, though they're not very square.`,
  p60_accessible_green: ({ n, m }) =>
    `There are ${n("n_qualifying_greens")} proper big greens, and ${m(/(\d+)% of buildings are within/)[1]}% of the buildings are within ${m(/within (\d+)m \(/)[1]} metres of one.`,
  p99_main_building: ({ n }) =>
    `There's one building that's clearly the big one, ${n("tallest_height_m")} metres tall and right near the middle. Everything else averages ${n("mean_other_height_m")}.`,
  p105_south_facing_outdoors: ({ m }) => `Only about half the outdoor spaces sit on the sunny side of their building. ${m(/(\d+)% have their nearest building to the north/)[1]}%.`,
  p121_path_shape: ({ n }) => `Every one of the ${n("n_streets")} paths widens out in the middle, like a place to linger.`,
  p160_building_edge: ({ n }) => `The buildings have interesting edges. ${n("n_buildings_with_real_wall_niches")} of the ${n("n_buildings")} even have a little nook in the wall.`,
  p52_network_of_paths_and_cars: () => `Where the footpaths cross the roads, they cross at odd angles. You'd want them square.`,
  p107_wings_of_light: ({ n }) =>
    `Most buildings are too fat. Only ${n("n_within_max_wing_width")} of ${n("n_buildings")} are thin enough to keep every room near an outside wall. They average ${n("mean_wing_width_m")} metres; Alexander says ${n("max_wing_width_m")}.`,
  p41_work_community: ({ n }) => `All ${n("n_near_square")} courtyard buildings are right next to a public square.`,
  p139_farmhouse_kitchen: ({ n }) => `The common rooms are big enough for a farmhouse kitchen, ${n("mean_common_cell_area_m2")} square metres on average.`,
  p161_sunny_place: ({ m }) => `Only ${m(/(\d+)% sit south of/)[1]}% of the outdoor spots are sunny ones, tucked on the south side of a building.`,
  p116_cascade_of_roofs: ({ n }) => `The roofs step down instead of staying flat, on all ${n("n_segmented_buildings")} buildings.`,
  p117_sheltering_roof: ({ n, m }) => `Proper pitched roofs on ${m(/; (\d+) \(\d+%\) are sloped/)[1]} of the ${n("n_roofed_buildings")} buildings.`,
  p118_roof_garden: ({ m }) => `Only ${m(/; (\d+) \(\d+%\) have at least one real, flat/)[1]} buildings have a roof you could actually go up and sit on.`,
  p119_arcades: ({ n, m }) => `Only ${m(/; (\d+) \(\d+%\) have at least one real ground-floor arcade/)[1]} of the ${n("n_buildings")} have a covered walk along the front. The rest, you get rained on.`,
  p162_north_face: ({ n, m }) => `The roofs slope down on the north side, ${m(/; (\d+) \(\d+%\) have a real asymmetric/)[1]} of ${n("n_roofed_buildings")} of them, so they cast less shadow.`,
  p166_gallery_surround: ({ n, m }) => `Only ${m(/; (\d+) \(\d+%\) have a real canopy/)[1]} of the ${n("n_multi_story_buildings")} buildings have a gallery or balcony on every floor.`,
  p51_green_streets: ({ n }) => `All ${n("n_local_streets")} little streets are grass with paving stones set in, not asphalt.`,
  p53_main_gateways: () => `There's a real way in where the paths cross the edge of the site.`,
  p126_something_roughly_in_the_middle: ({ n }) => `Every square has something in the middle of it. All ${n("n_plazas")}.`,
  p124_activity_pockets: ({ n }) => `Not one of the ${n("n_plazas")} squares has a little nook at its edge to hang out in.`,
  p195_staircase_volume: ({ m }) => `Only about half the staircases are a normal width, ${m(/(\d+)% fall within/)[1]}%, between 2 and 5 feet.`,
  p164_street_windows: ({ n }) => `Every building along a street has a window looking onto it. All ${n("n_street_adjacent_buildings")}.`,
  p197_thick_walls: ({ n }) => `Thick walls on all ${n("n_thick")} buildings. None of that flimsy stuff.`,
  p31_promenade: ({ n }) => `There's nowhere to promenade. None of the ${n("n_activity_nodes")} busy spots are strung together along a street.`,
  p59_quiet_backs: ({ n }) => `Most of the ${n("n_arterial_fronting_buildings")} buildings on the main road have a quiet side out back.`,
  p68_connected_play: ({ m }) => `Kids can get from one green to the next without crossing a big road. ${m(/(\d+)% connect without/)[1]}% of the time, anyway.`,

  // ---- abstentions: why they can't say
  p29_density_rings: () => `Does it get less crowded toward the edges? Can't tell. The blocks I'd measure got merged away.`,
  p37_house_cluster: () => `Do the houses cluster? Can't tell. The blocks they'd cluster in got merged away.`,
  p28_eccentric_nucleus: () => `Where's the middle of it all? Can't say. There are no blocks left to measure.`,
  p67_common_land: () => `Does each cluster get its own common land? No blocks left to check.`,
  p32_shopping_street: () => `A good shopping street? Nobody's marked any of the land for shops, so who knows.`,
  p46_market_of_many_shops: () => `A market of little shops? Nobody's marked any land for business yet.`,
  p89_corner_grocery: () => `A corner shop nearby? Nothing's marked for business, so I can't say.`,
  p33_night_life: () => `Night life? Nothing's marked for business yet.`,
  p88_street_cafe: () => `Street cafés? Nobody's marked any land for business, so I couldn't tell you.`,
  p93_food_stands: () => `Food stands? Nothing's marked for business yet.`,
  p48_housing_in_between: () => `Housing mixed in with everything else? Nobody's marked anywhere as housing yet.`,
  p123_pedestrian_density: () => `How crowded is it? No idea. Nobody's counting people.`,
  p147_communal_eating: () => `Do people eat together here? You can't see that in a floor plan.`,
  p198_closets_between_rooms: () => `Closets? This place doesn't even have a word for closets.`,
  p98_circulation_realms: () => `The different zones you pass through on the way in? Nobody's mapped them, so I can't say.`,
};

/** A person's version of what `op` found, or null when there isn't one (use the opinion's own words). */
export function plainFor(op) {
  const make = PLAIN[op.name];
  if (!make) return null;
  const d = op.details || {};
  const says = String(op.says || "");
  const n = (key) => {
    const x = parseFloat(d[key]);
    if (!Number.isFinite(x)) throw new Error(`no ${key}`);
    return x;
  };
  const m = (re) => {
    const r = re.exec(says);
    if (!r) throw new Error(`no ${re}`);
    return r;
  };
  try {
    const line = make({ n, m, f: fmt, says, value: op.value });
    return /undefined|NaN/.test(line) ? null : line;
  } catch {
    return null;
  }
}

// The chorus's questions for humans (street-smarts-conflict's build_report), plainly.
const QUESTIONS = [
  [/^The geometric chorus says this place is alive/, "It's laid out beautifully, but is it looking after the people who live in it? Which matters more?"],
  [/^The activist chorus likes this place's ownership pattern/, "Funny, though. Who owns it is great, how it's laid out, less so. What's that about?"],
  [/^Levels of Scale says .* but Strong Centers says/, "There's variety here, but does it have a heart? Or a heart and no variety. Which is it, would you say?"],
  [/^Levels of Scale has a view here; Strong Centers couldn't/, "So where's the heart of this place, would you say?"],
  [/^All voices roughly agree/, "We all pretty much agree. So what are we all missing?"],
];

/** A bystander's version of the chorus's question for humans, or null. */
export function plainQuestion(question) {
  const hit = QUESTIONS.find(([re]) => re.test(question));
  return hit ? hit[1] : null;
}

/** Opinions that have a plain line written for them (whether or not today's output can fill it). */
export const PLAIN_NAMES = Object.keys(PLAIN);
