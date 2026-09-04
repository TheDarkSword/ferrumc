//! The two random sources the game generates a world with.
//!
//! These are copied exactly rather than approximated, and the reason is not the terrain. A world is
//! only meant to be *vanilla-like* here — the same biomes in the same sorts of places, not the same
//! seed producing the same hill. But the same sources seed structures, features and loot tables,
//! and those have to be reproducible: two runs of the same server on the same seed must lay the
//! same chest down, or nothing that walks a world twice can be trusted.
//!
//! Two of them, because the game still carries both. The **legacy** one is `java.util.Random`, a
//! forty-eight bit congruential generator from the nineteen-nineties, kept because a great deal of
//! older generation was tuned around its exact output. The **modern** one is Xoroshiro128++, which
//! is what everything since the cave update seeds from.
//!
//! What matters as much as the generators is how a *position* becomes a source. Nothing walks the
//! world in order: a feature at one place has to be able to derive its own randomness without
//! knowing what else has been generated, which is what a positional factory is for.

/// The two halves a modern source is seeded from.
///
/// The pair is never both zero — a Xoroshiro state of nothing produces nothing forever — so a seed
/// that would be is replaced by two fixed values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seed128 {
    pub low: i64,
    pub high: i64,
}

/// What a whole word of bits is multiplied by to land between nothing and one.
///
/// Written as the game writes it — at *single* precision, then widened. The obvious constant is a
/// hair different, and a hair is enough for two implementations to disagree about which side of a
/// threshold a piece of terrain falls on.
const DOUBLE_STEP: f64 = 1.110_223e-16_f32 as f64;
const FLOAT_STEP: f32 = 5.960_464_5e-8;

/// The two constants the game seeds and stirs with, which are the golden and silver ratios written
/// as sixty-four bit integers.
const GOLDEN_RATIO: i64 = -7046029254386353131;
const SILVER_RATIO: i64 = 7640891576956012809;

impl Seed128 {
    /// Turns one number into two, the way the game widens an old seed.
    #[must_use]
    pub const fn upgrade(seed: i64) -> Self {
        let low = seed ^ SILVER_RATIO;
        let high = low.wrapping_add(GOLDEN_RATIO);
        Self {
            low: stir(low),
            high: stir(high),
        }
    }

    /// Both halves, exclusive-or'd with a pair.
    #[must_use]
    pub const fn xor(self, low: i64, high: i64) -> Self {
        Self {
            low: self.low ^ low,
            high: self.high ^ high,
        }
    }
}

/// Stafford's thirteenth mixer, which spreads the bits of a seed before it is used.
///
/// Without it two seeds a single bit apart start their sequences a single bit apart, and terrain
/// generated from neighbouring seeds looks alike.
#[must_use]
pub const fn stir(mut z: i64) -> i64 {
    z = (z ^ ((z as u64) >> 30) as i64).wrapping_mul(-4658895280553007687);
    z = (z ^ ((z as u64) >> 27) as i64).wrapping_mul(-7723592293110705685);
    z ^ ((z as u64) >> 31) as i64
}

/// How a position becomes a seed.
///
/// Deliberately cheap and deliberately not a hash: the game has used this since long before the
/// modern generator and the numbers it produces are baked into how terrain looks.
#[must_use]
pub const fn seed_at(x: i32, y: i32, z: i32) -> i64 {
    let mut seed =
        (x as i64).wrapping_mul(3_129_871) ^ (z as i64).wrapping_mul(116_129_781) ^ y as i64;
    seed = seed
        .wrapping_mul(seed)
        .wrapping_mul(42_317_861)
        .wrapping_add(seed.wrapping_mul(11));
    seed >> 16
}

/// Anything that produces the numbers a world is built from.
pub trait Random {
    /// The next sixty-four bits.
    fn next_long(&mut self) -> i64;

    /// The next `bits` bits, as the low bits of an integer.
    fn next_bits(&mut self, bits: u32) -> i32;

    /// A whole number below `bound`.
    fn next_int(&mut self, bound: i32) -> i32;

    /// A number from nothing up to but not including one.
    fn next_double(&mut self) -> f64;

    /// The same at single precision.
    fn next_float(&mut self) -> f32;

    /// A coin.
    fn next_bool(&mut self) -> bool;

    /// Steps the source on without using what it produces.
    ///
    /// The game does this to keep two sources that have been used differently in step, and skipping
    /// it puts everything after it somewhere else.
    fn skip(&mut self, rounds: u32) {
        for _ in 0..rounds {
            let _ = self.next_long();
        }
    }
}

/// Xoroshiro128++, which is what the world is seeded from now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xoroshiro {
    low: i64,
    high: i64,
}

impl Xoroshiro {
    /// From two halves. A pair of nothing would produce nothing forever, so it is replaced.
    #[must_use]
    pub const fn new(low: i64, high: i64) -> Self {
        if low | high == 0 {
            return Self {
                low: GOLDEN_RATIO,
                high: SILVER_RATIO,
            };
        }
        Self { low, high }
    }

    /// From one number, widened the way the game widens a world seed.
    #[must_use]
    pub const fn from_seed(seed: i64) -> Self {
        let widened = Seed128::upgrade(seed);
        Self::new(widened.low, widened.high)
    }

    #[must_use]
    pub const fn from_pair(seed: Seed128) -> Self {
        Self::new(seed.low, seed.high)
    }

    /// A source of its own, drawn from this one.
    pub fn fork(&mut self) -> Self {
        Self::new(self.next_long(), self.next_long())
    }

    /// A factory that turns a position into a source.
    pub fn fork_positional(&mut self) -> Positional {
        Positional {
            low: self.next_long(),
            high: self.next_long(),
        }
    }
}

impl Random for Xoroshiro {
    fn next_long(&mut self) -> i64 {
        let low = self.low;
        let mut high = self.high;
        let result = low.wrapping_add(high).rotate_left(17).wrapping_add(low);
        high ^= low;
        self.low = low.rotate_left(49) ^ high ^ (high << 21);
        self.high = high.rotate_left(28);
        result
    }

    fn next_bits(&mut self, bits: u32) -> i32 {
        ((self.next_long() as u64) >> (64 - bits)) as i32
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "a bound has to be positive");
        // Lemire's method, as the game writes it: multiply a full word by the bound and take the
        // top half, rejecting the few values that would make one bucket bigger than the others.
        let bound = bound as u32;
        let mut bits = u64::from(self.next_long() as u32);
        let mut multiplied = bits.wrapping_mul(u64::from(bound));
        let mut fraction = multiplied & 0xFFFF_FFFF;
        if fraction < u64::from(bound) {
            let smallest = u64::from((!bound).wrapping_add(1) % bound);
            while fraction < smallest {
                bits = u64::from(self.next_long() as u32);
                multiplied = bits.wrapping_mul(u64::from(bound));
                fraction = multiplied & 0xFFFF_FFFF;
            }
        }
        (multiplied >> 32) as i32
    }

    fn next_double(&mut self) -> f64 {
        self.next_bits_wide(53) as f64 * DOUBLE_STEP
    }

    fn next_float(&mut self) -> f32 {
        self.next_bits_wide(24) as f32 * FLOAT_STEP
    }

    fn next_bool(&mut self) -> bool {
        self.next_long() & 1 != 0
    }
}

impl Xoroshiro {
    /// The next `bits` bits, kept wide so a fifty-three bit shift is not truncated to an integer.
    fn next_bits_wide(&mut self, bits: u32) -> i64 {
        ((self.next_long() as u64) >> (64 - bits)) as i64
    }
}

/// A factory that turns a position into a source of its own.
///
/// This is what lets a feature at one place decide what it looks like without knowing anything
/// about what has already been generated: the position alone decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Positional {
    low: i64,
    high: i64,
}

impl Positional {
    /// The lower half of what it was seeded with.
    #[must_use]
    pub const fn low(&self) -> i64 {
        self.low
    }

    /// The upper half.
    #[must_use]
    pub const fn high(&self) -> i64 {
        self.high
    }

    /// The source belonging to one place.
    #[must_use]
    pub const fn at(&self, x: i32, y: i32, z: i32) -> Xoroshiro {
        Xoroshiro::new(seed_at(x, y, z) ^ self.low, self.high)
    }

    /// The source belonging to one number, which is how a structure derives its own.
    #[must_use]
    pub const fn from_seed(&self, seed: i64) -> Xoroshiro {
        Xoroshiro::new(seed ^ self.low, seed ^ self.high)
    }
}

/// `java.util.Random`: forty-eight bits of congruential generator from the nineteen-nineties.
///
/// Kept because a great deal of older generation was tuned around its exact output, and because
/// the noise the terrain is built from is still seeded through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Legacy {
    seed: i64,
}

/// What the state is multiplied by each step, and what is added. Both are the constants
/// `java.util.Random` has always used.
const LEGACY_MULTIPLIER: i64 = 25_214_903_917;
const LEGACY_INCREMENT: i64 = 11;

/// The state is only forty-eight bits wide, which is what makes this generator as weak as it is.
const LEGACY_MASK: i64 = (1 << 48) - 1;

impl Legacy {
    #[must_use]
    pub const fn new(seed: i64) -> Self {
        Self {
            seed: (seed ^ LEGACY_MULTIPLIER) & LEGACY_MASK,
        }
    }

    /// Starts it over from a seed.
    pub const fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ LEGACY_MULTIPLIER) & LEGACY_MASK;
    }
}

impl Random for Legacy {
    fn next_long(&mut self) -> i64 {
        let high = i64::from(self.next_bits(32));
        let low = i64::from(self.next_bits(32));
        (high << 32).wrapping_add(low)
    }

    fn next_bits(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(LEGACY_MULTIPLIER)
            .wrapping_add(LEGACY_INCREMENT)
            & LEGACY_MASK;
        ((self.seed as u64) >> (48 - bits)) as i32
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "a bound has to be positive");
        // A power of two is taken from the high bits, which are the good ones in a generator this
        // old — its low bits have a short period.
        if (bound as u32).is_power_of_two() {
            return ((i64::from(bound) * i64::from(self.next_bits(31))) >> 31) as i32;
        }
        loop {
            let bits = self.next_bits(31);
            let value = bits % bound;
            // Thrown away where the range does not divide evenly, so no value comes up oftener.
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    fn next_double(&mut self) -> f64 {
        let high = i64::from(self.next_bits(26)) << 27;
        let low = i64::from(self.next_bits(27));
        (high + low) as f64 * DOUBLE_STEP
    }

    fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 * FLOAT_STEP
    }

    fn next_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    fn skip(&mut self, rounds: u32) {
        for _ in 0..rounds {
            let _ = self.next_bits(32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one property everything else rests on: the same seed produces the same world.
    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let mut one = Xoroshiro::from_seed(12345);
        let mut other = Xoroshiro::from_seed(12345);
        for _ in 0..64 {
            assert_eq!(one.next_long(), other.next_long());
        }
    }

    #[test]
    fn two_seeds_a_bit_apart_do_not_look_alike() {
        // Which is what the mixer is for: without it, neighbouring seeds make neighbouring worlds.
        let mut one = Xoroshiro::from_seed(1);
        let mut other = Xoroshiro::from_seed(2);
        let alike = (0..32)
            .filter(|_| (one.next_long() ^ other.next_long()).count_ones() < 8)
            .count();
        assert_eq!(alike, 0, "the two sequences should share nothing");
    }

    /// A state of nothing would produce nothing forever.
    #[test]
    fn a_seed_of_nothing_is_replaced() {
        let mut nothing = Xoroshiro::new(0, 0);
        assert_ne!(nothing.next_long(), 0);
        assert_ne!(nothing.next_long(), 0);
    }

    /// The position alone decides, which is what lets a chunk be generated on its own.
    #[test]
    fn a_place_gets_the_same_source_however_it_is_reached() {
        let mut world = Xoroshiro::from_seed(99);
        let places = world.fork_positional();

        let mut once = places.at(10, 64, -20);
        let mut again = places.at(10, 64, -20);
        assert_eq!(once.next_long(), again.next_long());

        let mut elsewhere = places.at(11, 64, -20);
        assert_ne!(places.at(10, 64, -20).next_long(), elsewhere.next_long());
    }

    #[test]
    fn a_fork_goes_its_own_way() {
        let mut world = Xoroshiro::from_seed(7);
        let mut one = world.fork();
        let mut other = world.fork();
        assert_ne!(one.next_long(), other.next_long());
    }

    /// A bound is respected and every value inside it turns up.
    #[test]
    fn a_bounded_number_stays_inside_its_bound() {
        let mut random = Xoroshiro::from_seed(4);
        let mut seen = [false; 7];
        for _ in 0..2000 {
            let rolled = random.next_int(7);
            assert!((0..7).contains(&rolled), "{rolled}");
            seen[rolled as usize] = true;
        }
        assert!(seen.iter().all(|hit| *hit), "every value should turn up");
    }

    #[test]
    fn a_double_is_between_nothing_and_one() {
        let mut random = Xoroshiro::from_seed(11);
        for _ in 0..1000 {
            let rolled = random.next_double();
            assert!((0.0..1.0).contains(&rolled), "{rolled}");
        }
        let mut random = Legacy::new(11);
        for _ in 0..1000 {
            let rolled = random.next_double();
            assert!((0.0..1.0).contains(&rolled), "{rolled}");
        }
    }

    /// The numbers `java.util.Random` has always produced from a seed of nothing. Anyone can check
    /// these against a two-line Java program, which is the point of writing them down.
    #[test]
    fn the_legacy_source_is_javas_own() {
        let mut random = Legacy::new(0);
        assert_eq!(random.next_int(100), 60);
        assert_eq!(random.next_int(100), 48);
        assert_eq!(random.next_int(100), 29);

        let mut random = Legacy::new(0);
        assert_eq!(random.next_bits(32), -1_155_484_576);

        let mut random = Legacy::new(42);
        assert_eq!(random.next_int(100), 30);
        assert_eq!(random.next_int(100), 63);
        assert_eq!(random.next_int(100), 48);
    }

    #[test]
    fn the_legacy_source_repeats_from_a_seed() {
        let mut one = Legacy::new(-5);
        let mut other = Legacy::new(-5);
        for _ in 0..64 {
            assert_eq!(one.next_long(), other.next_long());
        }
    }

    /// Stepping a source on has to move it exactly as far as using it would.
    #[test]
    fn skipping_moves_a_source_as_far_as_using_it() {
        let mut skipped = Xoroshiro::from_seed(3);
        skipped.skip(5);

        let mut used = Xoroshiro::from_seed(3);
        for _ in 0..5 {
            let _ = used.next_long();
        }
        assert_eq!(skipped.next_long(), used.next_long());
    }

    /// The seed a place is given is the game's own arithmetic, which terrain has been shaped
    /// around for years.
    #[test]
    fn a_positional_seed_is_the_games_own() {
        assert_eq!(seed_at(0, 0, 0), 0);
        assert_ne!(seed_at(1, 0, 0), seed_at(0, 0, 1));
        assert_ne!(seed_at(0, 1, 0), seed_at(0, 0, 0));
    }
}
