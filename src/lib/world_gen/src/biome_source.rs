//! Which biome belongs at a place.
//!
//! Not a map and not a set of rules: a **nearest-neighbour lookup in six dimensions**. Each biome
//! claims one or more boxes in a space whose axes are temperature, humidity, continentalness,
//! erosion, depth and weirdness, and a place gets whichever box is nearest to where its climate
//! falls. Nothing owns a region of the world — a biome owns a region of *climate*, and the terrain
//! decides which climates turn up where.
//!
//! That is why biomes border plausibly without anyone saying they should: two biomes that neighbour
//! on the map are two boxes that neighbour in climate, and the smooth noise that produces the
//! climate cannot jump between distant ones.
//!
//! The distance to a box is nothing when the point is inside it, so a place well within a biome's
//! claim matches it outright and only the edges are actually contested.

use std::collections::HashMap;
use std::path::Path;

/// The six axes, plus the one that is not an axis.
///
/// `offset` is added to every distance as a flat penalty, which is how a biome is made to lose ties
/// it would otherwise win. It is not a coordinate: nothing is ever measured along it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Climate {
    pub temperature: f32,
    pub humidity: f32,
    pub continentalness: f32,
    pub erosion: f32,
    pub depth: f32,
    pub weirdness: f32,
}

impl Climate {
    #[must_use]
    pub const fn new(
        temperature: f32,
        humidity: f32,
        continentalness: f32,
        erosion: f32,
        depth: f32,
        weirdness: f32,
    ) -> Self {
        Self {
            temperature,
            humidity,
            continentalness,
            erosion,
            depth,
            weirdness,
        }
    }

    /// The six, in the order a claim lists them.
    const fn axes(&self) -> [f32; 6] {
        [
            self.temperature,
            self.humidity,
            self.continentalness,
            self.erosion,
            self.depth,
            self.weirdness,
        ]
    }
}

/// One stretch of one axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub lowest: f32,
    pub highest: f32,
}

impl Span {
    /// How far outside it a value falls, which is nothing when it is inside.
    #[must_use]
    pub fn away_from(&self, value: f32) -> f32 {
        let above = value - self.highest;
        if above > 0.0 {
            return above;
        }
        (self.lowest - value).max(0.0)
    }
}

/// One biome's claim on a stretch of climate.
#[derive(Debug, Clone)]
pub struct Claim {
    /// Which biome, by the number it travels as.
    pub biome: String,
    /// The six stretches it claims.
    pub spans: [Span; 6],
    /// A flat penalty added to every distance, which is how ties are broken.
    pub offset: f32,
}

impl Claim {
    /// How far a climate is from this claim.
    ///
    /// Squared, because comparing squares says the same thing as comparing distances and costs no
    /// square root. The offset is squared into it as a constant, exactly as the game does.
    #[must_use]
    pub fn away_from(&self, climate: &Climate) -> f32 {
        let axes = climate.axes();
        let mut total = 0.0;
        for (span, value) in self.spans.iter().zip(axes) {
            let away = span.away_from(value);
            total += away * away;
        }
        total + self.offset * self.offset
    }
}

/// A box holding everything under one branch, which is what lets a branch be skipped.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    spans: [Span; 6],
    /// The smallest penalty anything under this branch pays, since a penalty cannot be undone by
    /// being near.
    least_offset: f32,
}

impl Bounds {
    /// The smallest distance anything under this branch could be.
    ///
    /// Where that is already worse than the best found, nothing under it can beat it and the whole
    /// branch is skipped. This is the entire reason the tree is faster than the list.
    fn at_least_from(&self, climate: &Climate) -> f32 {
        let axes = climate.axes();
        let mut total = 0.0;
        for (span, value) in self.spans.iter().zip(axes) {
            let away = span.away_from(value);
            total += away * away;
        }
        total + self.least_offset * self.least_offset
    }
}

/// One branch of the search.
#[derive(Debug, Clone)]
enum Branch {
    /// A handful of claims, checked one by one.
    Few(Vec<usize>),
    /// Two halves, each with a box around it.
    Split {
        bounds: [Bounds; 2],
        halves: Box<[Branch; 2]>,
    },
}

/// How many claims are worth checking one by one rather than splitting again.
const FEW: usize = 8;

/// Every claim in one dimension.
///
/// The claims are kept in the order the report lists them, and the tree holds their places rather
/// than the claims themselves — so a tie is broken by that order however the tree is walked.
#[derive(Debug, Clone, Default)]
pub struct BiomeSource {
    claims: Vec<Claim>,
    search: Option<Branch>,
}

impl BiomeSource {
    /// Which biome belongs at a climate.
    ///
    /// The nearest claim, and the first of them where two are equally near — which is the order the
    /// report lists them in, so a tie goes the same way whichever branch was walked first.
    #[must_use]
    pub fn at(&self, climate: &Climate) -> Option<&str> {
        self.at_counting(climate, &mut 0)
    }

    /// The same, counting how many claims were actually looked at.
    ///
    /// What "fast" means here is how much of the list the search skips, and that is a number rather
    /// than a duration — a clock in a test suite that runs in parallel measures the machine.
    #[must_use]
    pub fn at_counting(&self, climate: &Climate, looked_at: &mut usize) -> Option<&str> {
        let search = self.search.as_ref()?;
        let mut best = (f32::MAX, usize::MAX);
        self.walk(search, climate, &mut best, looked_at);
        self.claims.get(best.1).map(|claim| claim.biome.as_str())
    }

    /// Walks one branch, skipping whatever cannot beat what has been found.
    fn walk(
        &self,
        branch: &Branch,
        climate: &Climate,
        best: &mut (f32, usize),
        looked_at: &mut usize,
    ) {
        match branch {
            Branch::Few(claims) => {
                *looked_at += claims.len();
                for at in claims {
                    let away = self.claims[*at].away_from(climate);
                    // Strictly nearer, or equally near and earlier in the report.
                    if away < best.0 || (away == best.0 && *at < best.1) {
                        *best = (away, *at);
                    }
                }
            }
            Branch::Split { bounds, halves } => {
                // The nearer half first, so the further one is likelier to be skipped outright.
                let (first, second) =
                    if bounds[0].at_least_from(climate) <= bounds[1].at_least_from(climate) {
                        (0, 1)
                    } else {
                        (1, 0)
                    };
                for half in [first, second] {
                    if bounds[half].at_least_from(climate) <= best.0 {
                        self.walk(&halves[half], climate, best, looked_at);
                    }
                }
            }
        }
    }

    /// How many claims there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.claims.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }

    /// Every biome that claims anything.
    #[must_use]
    pub fn biomes(&self) -> Vec<&str> {
        let mut named: Vec<&str> = self
            .claims
            .iter()
            .map(|claim| claim.biome.as_str())
            .collect();
        named.sort_unstable();
        named.dedup();
        named
    }

    /// Reads one dimension's claims, under a root.
    #[must_use]
    pub fn load(root: &Path, dimension: &str) -> Self {
        let from = root
            .join("assets/extracted/26.2/reports/biome_parameters/minecraft")
            .join(format!("{dimension}.json"));
        let Ok(text) = std::fs::read_to_string(&from) else {
            tracing::warn!("no biome parameters for {dimension}");
            return Self::default();
        };
        let Ok(read) = serde_json::from_str::<serde_json::Value>(&text) else {
            tracing::warn!("the biome parameters for {dimension} are not valid json");
            return Self::default();
        };

        let claims: Vec<Claim> = read
            .get("biomes")
            .and_then(serde_json::Value::as_array)
            .map(|entries| entries.iter().filter_map(read_claim).collect())
            .unwrap_or_default();
        Self::from_claims(claims)
    }

    /// Builds the search over a set of claims.
    #[must_use]
    pub fn from_claims(claims: Vec<Claim>) -> Self {
        let search = (!claims.is_empty()).then(|| split(&claims, (0..claims.len()).collect()));
        Self { claims, search }
    }
}

/// Splits a set of claims until each branch holds only a handful.
///
/// The axis with the widest spread is the one split on, at the middle of that spread: splitting on
/// a narrow axis puts claims on both sides of a line nothing distinguishes them by, and the boxes
/// end up overlapping so much that nothing can be skipped.
fn split(claims: &[Claim], mine: Vec<usize>) -> Branch {
    if mine.len() <= FEW {
        return Branch::Few(mine);
    }

    let bounds = bounds_of(claims, &mine);
    let widest = (0..6)
        .max_by(|one, other| {
            let width = |at: usize| bounds.spans[at].highest - bounds.spans[at].lowest;
            width(*one).total_cmp(&width(*other))
        })
        .unwrap_or(0);
    let middle = (bounds.spans[widest].lowest + bounds.spans[widest].highest) / 2.0;

    let centre_of = |at: usize| {
        let span = claims[at].spans[widest];
        (span.lowest + span.highest) / 2.0
    };
    let (low, high): (Vec<usize>, Vec<usize>) =
        mine.iter().partition(|at| centre_of(**at) < middle);

    // Everything landed on one side, which means the axis does not separate them. Cut the list in
    // half instead: an even split is worth more than a meaningful one here.
    let (low, high) = if low.is_empty() || high.is_empty() {
        let mut sorted = mine;
        sorted.sort_by(|one, other| centre_of(*one).total_cmp(&centre_of(*other)));
        let at = sorted.len() / 2;
        let high = sorted.split_off(at);
        (sorted, high)
    } else {
        (low, high)
    };

    Branch::Split {
        bounds: [bounds_of(claims, &low), bounds_of(claims, &high)],
        halves: Box::new([split(claims, low), split(claims, high)]),
    }
}

/// The box around a set of claims.
fn bounds_of(claims: &[Claim], mine: &[usize]) -> Bounds {
    let mut spans = [Span {
        lowest: f32::MAX,
        highest: f32::MIN,
    }; 6];
    let mut least_offset = f32::MAX;
    for at in mine {
        let claim = &claims[*at];
        for (axis, span) in spans.iter_mut().enumerate() {
            span.lowest = span.lowest.min(claim.spans[axis].lowest);
            span.highest = span.highest.max(claim.spans[axis].highest);
        }
        least_offset = least_offset.min(claim.offset.abs());
    }
    Bounds {
        spans,
        least_offset: if least_offset == f32::MAX {
            0.0
        } else {
            least_offset
        },
    }
}

/// The six axes, in the order the report writes them under.
const AXES: [&str; 6] = [
    "temperature",
    "humidity",
    "continentalness",
    "erosion",
    "depth",
    "weirdness",
];

/// One claim, as the report writes it.
fn read_claim(entry: &serde_json::Value) -> Option<Claim> {
    let biome = entry.get("biome")?.as_str()?.to_string();
    let parameters = entry.get("parameters")?;

    let mut spans = [Span {
        lowest: 0.0,
        highest: 0.0,
    }; 6];
    for (at, axis) in AXES.iter().enumerate() {
        spans[at] = read_span(parameters.get(axis)?)?;
    }

    Some(Claim {
        biome,
        spans,
        offset: parameters
            .get("offset")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32,
    })
}

/// One stretch, which the report writes as a pair or — where it is a single value — as a number.
fn read_span(value: &serde_json::Value) -> Option<Span> {
    if let Some(single) = value.as_f64() {
        let single = single as f32;
        return Some(Span {
            lowest: single,
            highest: single,
        });
    }
    let pair = value.as_array()?;
    Some(Span {
        lowest: pair.first()?.as_f64()? as f32,
        highest: pair.get(1)?.as_f64()? as f32,
    })
}

/// How often each biome turns up across a sample, which is what a distribution is checked with.
#[must_use]
pub fn tally<'a>(source: &'a BiomeSource, climates: &[Climate]) -> HashMap<&'a str, usize> {
    let mut counted = HashMap::new();
    for climate in climates {
        if let Some(biome) = source.at(climate) {
            *counted.entry(biome).or_insert(0) += 1;
        }
    }
    counted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("the crate sits three deep in the repository")
            .to_path_buf()
    }

    fn overworld() -> BiomeSource {
        BiomeSource::load(&root(), "overworld")
    }

    #[test]
    fn every_claim_the_report_makes_is_read() {
        let source = overworld();
        assert_eq!(source.len(), 7594);
        assert_eq!(source.biomes().len(), 55);
    }

    #[test]
    fn the_nether_has_its_own_much_smaller_list() {
        let nether = BiomeSource::load(&root(), "nether");
        assert_eq!(nether.biomes().len(), 5);
    }

    /// A place inside a claim is no distance from it at all, which is what makes the middle of a
    /// biome uncontested.
    #[test]
    fn a_climate_inside_a_claim_is_no_distance_from_it() {
        let claim = Claim {
            biome: "minecraft:plains".to_string(),
            spans: [Span {
                lowest: -0.5,
                highest: 0.5,
            }; 6],
            offset: 0.0,
        };
        assert_eq!(
            claim.away_from(&Climate::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0)),
            0.0
        );
        assert!(claim.away_from(&Climate::new(1.0, 0.0, 0.0, 0.0, 0.0, 0.0)) > 0.0);
    }

    /// And the offset is a penalty on everything, even from inside.
    #[test]
    fn an_offset_is_paid_wherever_the_climate_is() {
        let claim = Claim {
            biome: "minecraft:plains".to_string(),
            spans: [Span {
                lowest: -1.0,
                highest: 1.0,
            }; 6],
            offset: 0.5,
        };
        assert_eq!(
            claim.away_from(&Climate::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0)),
            0.25
        );
    }

    /// The distance along one axis is nothing inside and the overshoot outside.
    #[test]
    fn a_span_measures_only_the_overshoot() {
        let span = Span {
            lowest: -1.0,
            highest: 1.0,
        };
        assert_eq!(span.away_from(0.0), 0.0);
        assert_eq!(span.away_from(-1.0), 0.0, "the edge is inside");
        assert_eq!(span.away_from(1.0), 0.0);
        assert_eq!(span.away_from(1.5), 0.5);
        assert_eq!(span.away_from(-2.0), 1.0);
    }

    /// A climate taken from the middle of a biome's own claim finds that biome.
    ///
    /// The point is read off the report rather than guessed at: guessing which six numbers mean
    /// "desert" is exactly the sort of thing that produces a test asserting the wrong answer.
    #[test]
    fn the_middle_of_a_claim_finds_the_biome_that_claims_it() {
        let source = overworld();

        // The middle of one of the desert's own seven hundred claims.
        let desert = source
            .at(&Climate::new(0.775, -0.225, 0.515, 0.5, 0.0, -0.1583))
            .expect("something is nearest");
        assert_eq!(desert, "minecraft:desert");

        // Cold and well out to sea.
        let cold_sea = source
            .at(&Climate::new(-0.9, 0.0, -0.9, 0.0, 0.0, 0.0))
            .expect("something is nearest");
        assert!(cold_sea.contains("ocean"), "{cold_sea}");
    }

    /// Every claim finds its own biome from its own middle. Seven and a half thousand of them, and
    /// a single misread axis would put a great many of them somewhere else.
    #[test]
    fn every_claim_finds_itself_from_its_own_middle() {
        let source = overworld();
        let mut wrong = Vec::new();

        for claim in &source.claims {
            let middle = |at: usize| (claim.spans[at].lowest + claim.spans[at].highest) / 2.0;
            let here = Climate::new(
                middle(0),
                middle(1),
                middle(2),
                middle(3),
                middle(4),
                middle(5),
            );
            let found = source.at(&here).expect("something is nearest");
            // Overlapping claims are real — several biomes claim the same box at different
            // offsets — so what is checked is that the answer is *a* claim covering this point.
            let covers = source.claims.iter().any(|other| {
                other.biome == found && other.away_from(&here) <= claim.away_from(&here)
            });
            if !covers {
                wrong.push(claim.biome.clone());
            }
        }
        assert!(
            wrong.is_empty(),
            "{} claims found something further away",
            wrong.len()
        );
    }

    /// Mushroom fields are the one biome that claims a stretch of continentalness nothing else
    /// reaches, which is why they only turn up alone in the middle of an ocean.
    #[test]
    fn mushroom_fields_sit_where_nothing_else_does() {
        let source = overworld();
        let found = source
            .at(&Climate::new(0.0, 0.0, -1.1, 0.0, 0.0, 0.0))
            .expect("something is nearest");
        assert_eq!(found, "minecraft:mushroom_fields");
    }

    /// Something is always found, however far outside every claim the climate falls.
    #[test]
    fn a_climate_past_every_claim_still_gets_a_biome() {
        let source = overworld();
        assert!(
            source
                .at(&Climate::new(50.0, -50.0, 50.0, -50.0, 50.0, -50.0))
                .is_some()
        );
    }

    /// Neighbouring climates give the same biome or a neighbouring one, never a jump: which is
    /// what stops a desert bordering a snowy taiga.
    #[test]
    fn a_small_step_in_climate_is_a_small_step_in_biome() {
        let source = overworld();
        let mut changes = 0;
        let mut last = None;
        for step in 0..400 {
            let t = step as f32 / 200.0 - 1.0;
            let here = source
                .at(&Climate::new(t, 0.1, 0.3, -0.2, 0.0, 0.0))
                .expect("something is nearest");
            if last != Some(here) {
                changes += 1;
                last = Some(here);
            }
        }
        assert!(
            (1..12).contains(&changes),
            "walking one axis crossed {changes} biomes, which is too many to be smooth"
        );
    }

    /// The tree has to give the same answer the list would, or it is a faster way to be wrong.
    #[test]
    fn the_search_agrees_with_looking_at_every_claim() {
        let source = overworld();
        let mut random = crate::random::Xoroshiro::from_seed(2024);
        use crate::random::Random;

        for _ in 0..3000 {
            let mut axis = || (random.next_double() as f32) * 2.4 - 1.2;
            let here = Climate::new(axis(), axis(), axis(), axis(), axis(), axis());

            // What looking at all seven and a half thousand would say.
            let by_hand = source
                .claims
                .iter()
                .enumerate()
                .map(|(at, claim)| (claim.away_from(&here), at))
                .reduce(|best, next| {
                    if next.0 < best.0 || (next.0 == best.0 && next.1 < best.1) {
                        next
                    } else {
                        best
                    }
                })
                .map(|(_, at)| source.claims[at].biome.as_str());

            assert_eq!(source.at(&here), by_hand, "at {here:?}");
        }
    }

    /// And it has to skip most of the list, which is what makes it usable a thousand times a chunk.
    #[test]
    fn the_search_skips_most_of_the_claims() {
        let source = overworld();
        let mut worst = 0;
        let mut total = 0;
        let rounds = 2000;

        for step in 0..rounds {
            let t = step as f32 / (rounds as f32 / 2.0) - 1.0;
            let mut looked_at = 0;
            let _ = source.at_counting(
                &Climate::new(t, -t, t * 0.5, -t * 0.5, 0.0, t * 0.3),
                &mut looked_at,
            );
            worst = worst.max(looked_at);
            total += looked_at;
        }

        let average = total / rounds;
        assert!(
            average < source.len() / 20,
            "it looked at {average} of {} claims on average",
            source.len()
        );
        assert!(
            worst < source.len() / 4,
            "at its worst it looked at {worst} of {}",
            source.len()
        );
    }

    /// A dimension with no report reads as nothing rather than stopping the world.
    #[test]
    fn a_dimension_with_no_parameters_is_empty() {
        let missing = BiomeSource::load(&root(), "no_such_dimension");
        assert!(missing.is_empty());
        assert!(
            missing
                .at(&Climate::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0))
                .is_none()
        );
    }
}
