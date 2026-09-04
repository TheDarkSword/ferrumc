//! The noise the world's shape is built from.
//!
//! Three layers, each one built on the last:
//!
//! 1. **Perlin** — one lattice of gradients. Smooth, and on its own far too smooth to be terrain.
//! 2. **Octaves** — several of those at doubling frequencies and halving weights, which is what
//!    gives a hill both its shape and its bumps.
//! 3. **Normal** — two octave stacks laid over each other at a slight offset, then scaled so the
//!    result is roughly normally distributed. Everything downstream assumes that distribution when
//!    it compares against a threshold.
//!
//! Each octave seeds itself from the name `octave_<n>`, hashed. That is what lets one octave be
//! skipped where its amplitude is zero without moving every octave after it — which is the whole
//! reason a positional factory takes a name at all.

use crate::random::{Random, Xoroshiro, seed_at, stir};

/// The sixteen directions a gradient can point.
///
/// Twelve real ones and four repeats — the repeats are a wart of the original algorithm that
/// everything since has been tuned around, so they stay.
const GRADIENTS: [[i8; 3]; 16] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
    [1, 1, 0],
    [0, -1, 1],
    [-1, 1, 0],
    [0, -1, -1],
];

/// How far out a coordinate is allowed to run before it is folded back.
///
/// The lattice is only so large; past this the gradients would repeat anyway, and folding keeps
/// the arithmetic away from the range where a double stops being able to tell two places apart.
const FOLD_AT: f64 = 3.355_443_2e7;

/// Smoothstep: what turns a straight line between two lattice points into a curve.
///
/// Without it a Perlin lattice looks like a lattice — the joins show.
fn smooth(x: f64) -> f64 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

/// Folds a coordinate back into the range the lattice covers.
#[must_use]
pub fn fold(x: f64) -> f64 {
    x - (x / FOLD_AT + 0.5).floor() * FOLD_AT
}

/// One lattice of gradients.
#[derive(Debug, Clone)]
pub struct Perlin {
    /// The shuffled table that turns a lattice point into a gradient.
    permutation: [u8; 256],
    /// Where the lattice sits, so two of these seeded differently do not line up.
    origin: [f64; 3],
}

impl Perlin {
    /// A lattice seeded from a source.
    ///
    /// The offsets are drawn before the shuffle and the shuffle walks forwards, both of which
    /// matter: drawing them in another order gives a different lattice from the same seed.
    pub fn new(random: &mut impl Random) -> Self {
        let origin = [
            random.next_double() * 256.0,
            random.next_double() * 256.0,
            random.next_double() * 256.0,
        ];

        let mut permutation = [0u8; 256];
        for (at, slot) in permutation.iter_mut().enumerate() {
            *slot = at as u8;
        }
        for at in 0..256 {
            let step = random.next_int(256 - at as i32) as usize;
            permutation.swap(at, at + step);
        }

        Self {
            permutation,
            origin,
        }
    }

    /// The table, read the way the lattice wraps.
    fn p(&self, at: i32) -> i32 {
        i32::from(self.permutation[(at & 0xFF) as usize])
    }

    /// The noise at a place, between roughly minus one and one.
    #[must_use]
    pub fn at(&self, x: f64, y: f64, z: f64) -> f64 {
        let x = x + self.origin[0];
        let y = y + self.origin[1];
        let z = z + self.origin[2];

        let (xf, yf, zf) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
        let (xr, yr, zr) = (x - f64::from(xf), y - f64::from(yf), z - f64::from(zf));

        let x0 = self.p(xf);
        let x1 = self.p(xf + 1);
        let xy00 = self.p(x0 + yf);
        let xy01 = self.p(x0 + yf + 1);
        let xy10 = self.p(x1 + yf);
        let xy11 = self.p(x1 + yf + 1);

        // The eight corners of the cell this place falls in, each weighted by its own gradient.
        let corner = |hash: i32, x: f64, y: f64, z: f64| {
            let g = GRADIENTS[(hash & 15) as usize];
            f64::from(g[0]) * x + f64::from(g[1]) * y + f64::from(g[2]) * z
        };
        let d000 = corner(self.p(xy00 + zf), xr, yr, zr);
        let d100 = corner(self.p(xy10 + zf), xr - 1.0, yr, zr);
        let d010 = corner(self.p(xy01 + zf), xr, yr - 1.0, zr);
        let d110 = corner(self.p(xy11 + zf), xr - 1.0, yr - 1.0, zr);
        let d001 = corner(self.p(xy00 + zf + 1), xr, yr, zr - 1.0);
        let d101 = corner(self.p(xy10 + zf + 1), xr - 1.0, yr, zr - 1.0);
        let d011 = corner(self.p(xy01 + zf + 1), xr, yr - 1.0, zr - 1.0);
        let d111 = corner(self.p(xy11 + zf + 1), xr - 1.0, yr - 1.0, zr - 1.0);

        let (xa, ya, za) = (smooth(xr), smooth(yr), smooth(zr));
        lerp(
            za,
            lerp(ya, lerp(xa, d000, d100), lerp(xa, d010, d110)),
            lerp(ya, lerp(xa, d001, d101), lerp(xa, d011, d111)),
        )
    }
}

/// How strongly each octave counts, and where the first one sits.
///
/// The first octave is usually negative: octave -7 is a very slow wave that decides where the
/// continents are, and octave 0 is one wave a block.
#[derive(Debug, Clone, PartialEq)]
pub struct Octaves {
    pub first: i32,
    pub amplitudes: Vec<f64>,
}

impl Octaves {
    #[must_use]
    pub fn new(first: i32, amplitudes: Vec<f64>) -> Self {
        Self { first, amplitudes }
    }
}

/// Several lattices at doubling frequencies.
///
/// An octave whose amplitude is zero is not built at all — which is why each one seeds itself from
/// its own name rather than from whatever the source happened to be after the last.
#[derive(Debug, Clone)]
pub struct Layered {
    levels: Vec<Option<Perlin>>,
    amplitudes: Vec<f64>,
    /// How much the input is scaled at the lowest octave.
    lowest_input: f64,
    /// How much the output is scaled at the lowest octave.
    lowest_value: f64,
    highest: f64,
}

impl Layered {
    /// Builds the stack a set of octaves asks for.
    pub fn new(random: &mut Xoroshiro, octaves: &Octaves) -> Self {
        let places = random.fork_positional();
        let count = octaves.amplitudes.len();
        let zeroth = -octaves.first;

        let levels = octaves
            .amplitudes
            .iter()
            .enumerate()
            .map(|(at, amplitude)| {
                if *amplitude == 0.0 {
                    return None;
                }
                let octave = octaves.first + at as i32;
                // Named rather than numbered, so leaving one out moves nothing else.
                let mut own = places.from_hash_of(&format!("octave_{octave}"));
                Some(Perlin::new(&mut own))
            })
            .collect();

        let lowest_input = 2.0f64.powi(-zeroth);
        let lowest_value = 2.0f64.powi(count as i32 - 1) / (2.0f64.powi(count as i32) - 1.0);

        let mut built = Self {
            levels,
            amplitudes: octaves.amplitudes.clone(),
            lowest_input,
            lowest_value,
            highest: 0.0,
        };
        built.highest = built.edge(2.0);
        built
    }

    /// The most the stack can produce, which is what the layer above scales by.
    #[must_use]
    pub const fn highest(&self) -> f64 {
        self.highest
    }

    /// What the stack would come to if every octave were at a given value.
    fn edge(&self, value: f64) -> f64 {
        let mut total = 0.0;
        let mut factor = self.lowest_value;
        for (at, level) in self.levels.iter().enumerate() {
            if level.is_some() {
                total += self.amplitudes[at] * value * factor;
            }
            factor /= 2.0;
        }
        total
    }

    /// The noise at a place.
    #[must_use]
    pub fn at(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut total = 0.0;
        let mut input = self.lowest_input;
        let mut factor = self.lowest_value;

        for (at, level) in self.levels.iter().enumerate() {
            if let Some(level) = level {
                total += self.amplitudes[at]
                    * level.at(fold(x * input), fold(y * input), fold(z * input))
                    * factor;
            }
            input *= 2.0;
            factor /= 2.0;
        }
        total
    }
}

/// How far apart the two stacks are sampled.
///
/// A hair over one: sampling them at exactly the same place would have them rise and fall together,
/// which is the opposite of what laying two over each other is for.
const OFFSET: f64 = 1.018_126_888_217_522_7;

/// Two octave stacks laid over each other, scaled to a known spread.
///
/// This is what the rest of world generation actually asks. The scaling matters as much as the
/// noise: everything downstream compares the result against a threshold, and a threshold means
/// nothing unless the spread is known.
#[derive(Debug, Clone)]
pub struct Noise {
    first: Layered,
    second: Layered,
    factor: f64,
    highest: f64,
}

/// What the spread is scaled to.
const TARGET: f64 = 1.0 / 6.0;

impl Noise {
    /// Builds one from a set of octaves.
    pub fn new(random: &mut Xoroshiro, octaves: &Octaves) -> Self {
        let first = Layered::new(random, octaves);
        let second = Layered::new(random, octaves);

        // How far apart the octaves that count actually are, which is what decides how wide the
        // raw spread is before it is scaled.
        let used: Vec<usize> = octaves
            .amplitudes
            .iter()
            .enumerate()
            .filter(|(_, amplitude)| **amplitude != 0.0)
            .map(|(at, _)| at)
            .collect();
        let span = match (used.first(), used.last()) {
            (Some(lowest), Some(highest)) => (highest - lowest) as i32,
            _ => 0,
        };
        let factor = TARGET / (0.1 * (1.0 + 1.0 / f64::from(span + 1)));

        let highest = (first.highest() + second.highest()) * factor;
        Self {
            first,
            second,
            factor,
            highest,
        }
    }

    /// The noise at a place, roughly normally distributed about nothing.
    #[must_use]
    pub fn at(&self, x: f64, y: f64, z: f64) -> f64 {
        (self.first.at(x, y, z) + self.second.at(x * OFFSET, y * OFFSET, z * OFFSET)) * self.factor
    }

    /// The most it can produce.
    #[must_use]
    pub const fn highest(&self) -> f64 {
        self.highest
    }
}

/// What a name hashes to, as a pair of halves.
///
/// The game uses MD5 for this. Nothing about it needs to be a *good* hash — it needs to be the
/// same hash every time and to spread names apart, and the only reason to match the game's choice
/// exactly would be seed-for-seed terrain, which is not the aim.
fn hash_of(name: &str) -> (i64, i64) {
    // FNV-1a over the bytes, twice with different offsets, then stirred. Cheap, deterministic, and
    // it puts two names that differ by one character a long way apart.
    let one = fnv(name.as_bytes(), 0xcbf2_9ce4_8422_2325);
    let other = fnv(name.as_bytes(), 0x9e37_79b9_7f4a_7c15);
    (stir(one as i64), stir(other as i64))
}

fn fnv(bytes: &[u8], start: u64) -> u64 {
    let mut hash = start;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl crate::random::Positional {
    /// The source belonging to a name.
    ///
    /// What lets an octave seed itself from `octave_-7` rather than from its place in a list, so
    /// leaving one out does not move every octave after it.
    #[must_use]
    pub fn from_hash_of(&self, name: &str) -> Xoroshiro {
        let (low, high) = hash_of(name);
        Xoroshiro::new(low ^ self.low(), high ^ self.high())
    }
}

/// Where a place sits, for anything that wants a seed of its own without a factory.
#[must_use]
pub fn seed_for(x: i32, y: i32, z: i32) -> i64 {
    seed_at(x, y, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some_octaves() -> Octaves {
        Octaves::new(-7, vec![1.0, 1.0, 1.0, 1.0])
    }

    /// The one property the whole world rests on.
    #[test]
    fn the_same_seed_gives_the_same_noise() {
        let one = Noise::new(&mut Xoroshiro::from_seed(42), &some_octaves());
        let other = Noise::new(&mut Xoroshiro::from_seed(42), &some_octaves());
        for at in 0..50 {
            let x = f64::from(at) * 3.7;
            assert_eq!(one.at(x, 64.0, -x), other.at(x, 64.0, -x));
        }
    }

    #[test]
    fn two_seeds_give_different_noise() {
        let one = Noise::new(&mut Xoroshiro::from_seed(1), &some_octaves());
        let other = Noise::new(&mut Xoroshiro::from_seed(2), &some_octaves());
        let same = (0..50)
            .filter(|at| {
                let x = f64::from(*at) * 3.7;
                (one.at(x, 64.0, 0.0) - other.at(x, 64.0, 0.0)).abs() < 1e-9
            })
            .count();
        assert_eq!(same, 0);
    }

    /// Noise that is not smooth is not terrain: two places a hair apart have to be a hair apart.
    #[test]
    fn noise_is_smooth() {
        let noise = Noise::new(&mut Xoroshiro::from_seed(7), &some_octaves());
        let mut worst: f64 = 0.0;
        for at in 0..200 {
            let x = f64::from(at) * 0.01;
            worst = worst.max((noise.at(x, 0.0, 0.0) - noise.at(x + 0.001, 0.0, 0.0)).abs());
        }
        assert!(worst < 0.05, "a step of a thousandth moved it by {worst}");
    }

    /// And noise that is flat is not terrain either.
    #[test]
    fn noise_actually_varies() {
        let noise = Noise::new(&mut Xoroshiro::from_seed(7), &some_octaves());
        let sampled: Vec<f64> = (0..200)
            .map(|at| noise.at(f64::from(at) * 7.3, 0.0, 0.0))
            .collect();
        let lowest = sampled.iter().copied().fold(f64::MAX, f64::min);
        let highest = sampled.iter().copied().fold(f64::MIN, f64::max);
        assert!(
            highest - lowest > 0.5,
            "it barely moved: {lowest} to {highest}"
        );
    }

    /// Roughly normal about nothing, which is what everything downstream assumes when it compares
    /// against a threshold.
    #[test]
    fn noise_is_centred_on_nothing_and_mostly_inside_one() {
        let noise = Noise::new(&mut Xoroshiro::from_seed(3), &some_octaves());
        let sampled: Vec<f64> = (0..4000)
            .map(|at| {
                let t = f64::from(at) * 0.37;
                noise.at(t, t * 0.5, -t)
            })
            .collect();

        let mean = sampled.iter().sum::<f64>() / sampled.len() as f64;
        assert!(mean.abs() < 0.05, "it should sit about nothing, not {mean}");

        let outside = sampled.iter().filter(|value| value.abs() > 1.0).count();
        assert!(
            outside * 100 < sampled.len(),
            "{outside} of {} were past one",
            sampled.len()
        );
    }

    /// It never produces more than it says it can, which is what lets a caller normalise.
    #[test]
    fn nothing_comes_out_past_what_it_says_it_can() {
        let noise = Noise::new(&mut Xoroshiro::from_seed(5), &some_octaves());
        let ceiling = noise.highest();
        assert!(ceiling > 0.0);
        for at in 0..3000 {
            let t = f64::from(at) * 0.41;
            assert!(noise.at(t, -t, t * 2.0).abs() <= ceiling, "at {t}");
        }
    }

    /// An octave with no weight is not built, and leaving it out moves nothing else.
    #[test]
    fn a_silent_octave_does_not_shift_the_others() {
        let full = Octaves::new(-3, vec![1.0, 1.0, 1.0]);
        let middle_out = Octaves::new(-3, vec![1.0, 0.0, 1.0]);

        let with = Layered::new(&mut Xoroshiro::from_seed(9), &full);
        let without = Layered::new(&mut Xoroshiro::from_seed(9), &middle_out);

        // The first and last octaves are seeded by name, so they are the same lattice in both.
        // Only the middle one's contribution should be missing.
        let here = |stack: &Layered| stack.at(11.0, 5.0, -3.0);
        assert_ne!(here(&with), here(&without));

        let again = Layered::new(&mut Xoroshiro::from_seed(9), &middle_out);
        assert_eq!(
            here(&without),
            here(&again),
            "and it is still deterministic"
        );
    }

    /// A coordinate far from nothing is folded back rather than losing precision.
    #[test]
    fn a_far_away_place_is_folded_back() {
        assert_eq!(fold(0.0), 0.0);
        assert!(fold(1e9).abs() <= FOLD_AT / 2.0);
        assert!(fold(-1e9).abs() <= FOLD_AT / 2.0);
    }

    /// Two names get two different sources, and the same name gets the same one.
    #[test]
    fn a_name_gets_its_own_source() {
        let mut world = Xoroshiro::from_seed(4);
        let places = world.fork_positional();

        assert_eq!(
            places.from_hash_of("octave_-7").next_long(),
            places.from_hash_of("octave_-7").next_long()
        );
        assert_ne!(
            places.from_hash_of("octave_-7").next_long(),
            places.from_hash_of("octave_-6").next_long()
        );
    }
}
