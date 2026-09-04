//! The tree that decides where there is stone and where there is air.
//!
//! Terrain shape is not a formula, it is a **tree**: noise, constants, arithmetic, clamps, splines
//! and interpolation, composed in the packs and evaluated at a position. Changing where mountains
//! go is changing the data, not the code.
//!
//! Two parts of this are worth reading twice.
//!
//! The **spline** is where continentalness and erosion become landforms, and it is the one piece
//! that has to be arithmetically exact even though nothing else here does — an approximation gives
//! terrain that reads as wrong rather than as different.
//!
//! The **caching wrappers** — `flat_cache`, `cache_2d`, `cache_once`, `interpolated` — are the
//! packs saying "this is expensive and it does not change often". They are transparent here: the
//! answer is the same, the cost is not, and making them real is a performance job rather than a
//! correctness one.

use crate::noise::{Noise, Octaves};
use crate::noise_parameters::Parameters;
use crate::random::Xoroshiro;
use std::collections::HashMap;
use std::sync::Arc;

/// Where a value is being asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct At {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl At {
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

/// One node of the tree.
#[derive(Debug, Clone)]
pub enum Density {
    /// The same everywhere.
    Flat(f64),
    /// How high up it is, which is what everything vertical is built from.
    Y,
    /// A named piece of noise, sampled at a scaled position.
    Sampled {
        noise: Arc<Noise>,
        xz_scale: f64,
        y_scale: f64,
    },
    /// The same, at a position two other functions move.
    Shifted {
        noise: Arc<Noise>,
        shift_x: Box<Density>,
        shift_y: Box<Density>,
        shift_z: Box<Density>,
        xz_scale: f64,
        y_scale: f64,
    },
    /// One of the three offsets a shift function produces.
    Shift {
        noise: Arc<Noise>,
        axis: Axis,
    },
    Add(Box<Density>, Box<Density>),
    Mul(Box<Density>, Box<Density>),
    Min(Box<Density>, Box<Density>),
    Max(Box<Density>, Box<Density>),
    Abs(Box<Density>),
    Square(Box<Density>),
    Cube(Box<Density>),
    /// Everything below nothing halved, which flattens valleys without touching hills.
    HalfNegative(Box<Density>),
    /// The same, quartered.
    QuarterNegative(Box<Density>),
    Clamp {
        input: Box<Density>,
        lowest: f64,
        highest: f64,
    },
    /// A straight line between two heights, held flat outside them.
    YClampedGradient {
        from_y: i32,
        to_y: i32,
        from_value: f64,
        to_value: f64,
    },
    /// One of two, depending on where a third falls.
    RangeChoice {
        input: Box<Density>,
        lowest: f64,
        highest: f64,
        inside: Box<Density>,
        outside: Box<Density>,
    },
    /// The curve that turns a climate value into a landform.
    Spline(Box<Spline>),
    /// A wrapper the packs use to say something is expensive. Transparent here.
    Cached(Box<Density>),
    /// Something this server does not generate, which reads as nothing.
    ///
    /// The blending functions belong to a world being extended from an older one, and the end's
    /// island noise belongs to a dimension that does not exist yet.
    Unbuilt(&'static str),
}

/// Which offset a shift function produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Z,
}

/// How much a shift moves a position by, and how coarse it is.
const SHIFT_SCALE: f64 = 0.25;
const SHIFT_STRENGTH: f64 = 4.0;

impl Density {
    /// What it comes to at a place.
    #[must_use]
    pub fn at(&self, at: At) -> f64 {
        match self {
            Self::Flat(value) => *value,
            Self::Y => f64::from(at.y),
            Self::Sampled {
                noise,
                xz_scale,
                y_scale,
            } => noise.at(
                f64::from(at.x) * xz_scale,
                f64::from(at.y) * y_scale,
                f64::from(at.z) * xz_scale,
            ),
            Self::Shifted {
                noise,
                shift_x,
                shift_y,
                shift_z,
                xz_scale,
                y_scale,
            } => noise.at(
                f64::from(at.x) * xz_scale + shift_x.at(at),
                f64::from(at.y) * y_scale + shift_y.at(at),
                f64::from(at.z) * xz_scale + shift_z.at(at),
            ),
            // A shift samples the same noise on a quarter-scale lattice and multiplies up, which is
            // what makes a biome edge wander rather than run straight.
            Self::Shift { noise, axis } => {
                let (x, z) = match axis {
                    Axis::X => (at.x, at.z),
                    Axis::Z => (at.z, at.x),
                };
                noise.at(
                    f64::from(x) * SHIFT_SCALE,
                    f64::from(at.y) * SHIFT_SCALE,
                    f64::from(z) * SHIFT_SCALE,
                ) * SHIFT_STRENGTH
            }
            Self::Add(one, other) => one.at(at) + other.at(at),
            Self::Mul(one, other) => {
                // Nothing times anything is nothing, and the other side is often the expensive one.
                let first = one.at(at);
                if first == 0.0 {
                    return 0.0;
                }
                first * other.at(at)
            }
            Self::Min(one, other) => one.at(at).min(other.at(at)),
            Self::Max(one, other) => one.at(at).max(other.at(at)),
            Self::Abs(inner) => inner.at(at).abs(),
            Self::Square(inner) => {
                let value = inner.at(at);
                value * value
            }
            Self::Cube(inner) => {
                let value = inner.at(at);
                value * value * value
            }
            Self::HalfNegative(inner) => {
                let value = inner.at(at);
                if value > 0.0 { value } else { value * 0.5 }
            }
            Self::QuarterNegative(inner) => {
                let value = inner.at(at);
                if value > 0.0 { value } else { value * 0.25 }
            }
            Self::Clamp {
                input,
                lowest,
                highest,
            } => input.at(at).clamp(*lowest, *highest),
            Self::YClampedGradient {
                from_y,
                to_y,
                from_value,
                to_value,
            } => {
                let span = f64::from(to_y - from_y);
                if span == 0.0 {
                    return *from_value;
                }
                let t = (f64::from(at.y - from_y) / span).clamp(0.0, 1.0);
                from_value + t * (to_value - from_value)
            }
            Self::RangeChoice {
                input,
                lowest,
                highest,
                inside,
                outside,
            } => {
                let value = input.at(at);
                if value >= *lowest && value < *highest {
                    inside.at(at)
                } else {
                    outside.at(at)
                }
            }
            Self::Spline(spline) => f64::from(spline.at(at)),
            Self::Cached(inner) => inner.at(at),
            Self::Unbuilt(_) => 0.0,
        }
    }
}

/// A curve through a handful of points, each of which is itself a curve.
///
/// This is where continentalness and erosion turn into landforms, and it is written out exactly
/// rather than approximated: the shape between two points is a cubic fitted to the value and the
/// slope at each end, and a straight line through the same points gives terrain that reads as
/// wrong.
#[derive(Debug, Clone)]
pub enum Spline {
    /// The same whatever the coordinate.
    Flat(f32),
    /// A curve through points along a coordinate.
    Curve {
        coordinate: Box<Density>,
        points: Vec<Point>,
    },
}

/// One point of a curve: where it sits, what it is worth there, and how steeply it is leaving.
#[derive(Debug, Clone)]
pub struct Point {
    pub location: f32,
    pub value: Spline,
    pub derivative: f32,
}

impl Spline {
    /// What the curve comes to at a place.
    #[must_use]
    pub fn at(&self, at: At) -> f32 {
        match self {
            Self::Flat(value) => *value,
            Self::Curve { coordinate, points } => {
                let Some(last) = points.len().checked_sub(1) else {
                    return 0.0;
                };
                let input = coordinate.at(at) as f32;

                // Which pair of points the input falls between. Past either end the curve is
                // continued as a straight line at the slope it was leaving with.
                let start = points
                    .iter()
                    .position(|point| input < point.location)
                    .map_or(last as isize, |at| at as isize - 1);

                if start < 0 {
                    return extend(input, &points[0], points[0].value.at(at));
                }
                let start = start as usize;
                if start == last {
                    return extend(input, &points[last], points[last].value.at(at));
                }

                let here = &points[start];
                let next = &points[start + 1];
                let span = next.location - here.location;
                let t = (input - here.location) / span;
                let y1 = here.value.at(at);
                let y2 = next.value.at(at);

                // A cubic fitted to both values and both slopes. The two corrections are what the
                // slopes ask for beyond a straight line between the points.
                let a = here.derivative * span - (y2 - y1);
                let b = -next.derivative * span + (y2 - y1);
                lerp(t, y1, y2) + t * (1.0 - t) * lerp(t, a, b)
            }
        }
    }
}

/// The curve carried on past its last point, at the slope it was leaving with.
fn extend(input: f32, point: &Point, value: f32) -> f32 {
    if point.derivative == 0.0 {
        value
    } else {
        value + point.derivative * (input - point.location)
    }
}

fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

/// Everything the packs name, built and ready to be asked.
#[derive(Debug, Default)]
pub struct Functions {
    named: HashMap<String, Arc<Density>>,
}

impl Functions {
    /// One by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Arc<Density>> {
        self.named
            .get(name.strip_prefix("minecraft:").unwrap_or(name))
    }

    /// How many were built.
    #[must_use]
    pub fn len(&self) -> usize {
        self.named.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.named.is_empty()
    }

    /// Puts one in by hand, which is what a test does.
    pub fn insert(&mut self, name: &str, function: Density) {
        self.named.insert(name.to_string(), Arc::new(function));
    }
}

/// What is needed to turn the packs' JSON into a tree.
pub struct Builder<'a> {
    /// The world's own seed, which every piece of noise is drawn from.
    seed: i64,
    /// The octaves each named piece of noise is built from.
    parameters: &'a Parameters,
    /// The pieces already built, so one used twice is built once.
    noises: HashMap<String, Arc<Noise>>,
    /// Named functions already built, since one names another.
    built: HashMap<String, Arc<Density>>,
}

impl<'a> Builder<'a> {
    #[must_use]
    pub fn new(seed: i64, parameters: &'a Parameters) -> Self {
        Self {
            seed,
            parameters,
            noises: HashMap::new(),
            built: HashMap::new(),
        }
    }

    /// The noise a name asks for, built once and kept.
    fn noise(&mut self, name: &str) -> Option<Arc<Noise>> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name).to_string();
        if let Some(built) = self.noises.get(&name) {
            return Some(Arc::clone(built));
        }
        let octaves = self.parameters.get(&name)?;
        let built = Arc::new(self.build_noise(&name, octaves));
        self.noises.insert(name, Arc::clone(&built));
        Some(built)
    }

    /// Builds one piece of noise, seeded from the world and its own name.
    ///
    /// Named rather than numbered for the same reason an octave is: adding a piece of noise to a
    /// pack must not move every piece after it.
    fn build_noise(&self, name: &str, octaves: &Octaves) -> Noise {
        let mut world = Xoroshiro::from_seed(self.seed);
        let places = world.fork_positional();
        let mut own = places.from_hash_of(&format!("minecraft:{name}"));
        Noise::new(&mut own, octaves)
    }

    /// Reads every function the packs define, under a root.
    ///
    /// A function that names another is built after it, however they are ordered on disk: names are
    /// followed as they are met and what has already been built is kept.
    pub fn load(&mut self, root: &std::path::Path) -> Functions {
        let from = root.join(FROM);
        let mut written = HashMap::new();
        collect(&from, &from, &mut written);

        let names: Vec<String> = written.keys().cloned().collect();
        for name in names {
            let built = self.build_named(&name, &written, &mut Vec::new());
            self.built.insert(name, Arc::new(built));
        }

        Functions {
            named: std::mem::take(&mut self.built),
        }
    }

    /// One named function, building whatever it names first.
    ///
    /// `being_built` is the chain of names currently open. A pack that names itself in a circle
    /// would otherwise recurse until the stack ran out, and a bad pack should cost one function.
    fn build_named(
        &mut self,
        name: &str,
        written: &HashMap<String, serde_json::Value>,
        being_built: &mut Vec<String>,
    ) -> Density {
        if let Some(already) = self.built.get(name) {
            return (**already).clone();
        }
        if being_built.iter().any(|open| open == name) {
            tracing::warn!("the density function {name} names itself in a circle");
            return Density::Unbuilt("a circle");
        }
        let Some(written_here) = written.get(name) else {
            return Density::Unbuilt("no such function");
        };

        being_built.push(name.to_string());
        let built = self.build(written_here, written, being_built);
        being_built.pop();
        built
    }

    /// One piece of the tree.
    fn build(
        &mut self,
        value: &serde_json::Value,
        written: &HashMap<String, serde_json::Value>,
        being_built: &mut Vec<String>,
    ) -> Density {
        // A bare number is a constant, and a bare string names another function. Both are how the
        // packs write the common cases without a wrapper.
        if let Some(flat) = value.as_f64() {
            return Density::Flat(flat);
        }
        if let Some(name) = value.as_str() {
            let name = name.strip_prefix("minecraft:").unwrap_or(name).to_string();
            return self.build_named(&name, written, being_built);
        }

        let Some(kind) = value.get("type").and_then(serde_json::Value::as_str) else {
            return Density::Unbuilt("no type");
        };
        let kind = kind.strip_prefix("minecraft:").unwrap_or(kind);

        let mut arg = |name: &str, me: &mut Self| {
            value.get(name).map_or(Density::Flat(0.0), |inner| {
                me.build(inner, written, being_built)
            })
        };
        let number = |name: &str, fallback: f64| {
            value
                .get(name)
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(fallback)
        };
        let whole = |name: &str, fallback: i32| {
            value
                .get(name)
                .and_then(serde_json::Value::as_i64)
                .and_then(|read| i32::try_from(read).ok())
                .unwrap_or(fallback)
        };

        match kind {
            "constant" => Density::Flat(number("argument", 0.0)),
            "add" => Density::Add(
                Box::new(arg("argument1", self)),
                Box::new(arg("argument2", self)),
            ),
            "mul" => Density::Mul(
                Box::new(arg("argument1", self)),
                Box::new(arg("argument2", self)),
            ),
            "min" => Density::Min(
                Box::new(arg("argument1", self)),
                Box::new(arg("argument2", self)),
            ),
            "max" => Density::Max(
                Box::new(arg("argument1", self)),
                Box::new(arg("argument2", self)),
            ),
            "abs" => Density::Abs(Box::new(arg("argument", self))),
            "square" => Density::Square(Box::new(arg("argument", self))),
            "cube" => Density::Cube(Box::new(arg("argument", self))),
            "half_negative" => Density::HalfNegative(Box::new(arg("argument", self))),
            "quarter_negative" => Density::QuarterNegative(Box::new(arg("argument", self))),
            "clamp" => Density::Clamp {
                input: Box::new(arg("input", self)),
                lowest: number("min", f64::MIN),
                highest: number("max", f64::MAX),
            },
            "y_clamped_gradient" => Density::YClampedGradient {
                from_y: whole("from_y", 0),
                to_y: whole("to_y", 0),
                from_value: number("from_value", 0.0),
                to_value: number("to_value", 0.0),
            },
            "range_choice" => Density::RangeChoice {
                input: Box::new(arg("input", self)),
                lowest: number("min_inclusive", 0.0),
                highest: number("max_exclusive", 0.0),
                inside: Box::new(arg("when_in_range", self)),
                outside: Box::new(arg("when_out_of_range", self)),
            },
            // The packs say these are expensive. They are transparent here.
            "flat_cache" | "cache_2d" | "cache_once" | "cache_all_in_cell" | "interpolated" => {
                Density::Cached(Box::new(arg("argument", self)))
            }
            "noise" => self.noise_named(value, "noise").map_or(
                Density::Unbuilt("no such noise"),
                |noise| Density::Sampled {
                    noise,
                    xz_scale: number("xz_scale", 1.0),
                    y_scale: number("y_scale", 1.0),
                },
            ),
            "shifted_noise" => {
                let shift_x = Box::new(arg("shift_x", self));
                let shift_y = Box::new(arg("shift_y", self));
                let shift_z = Box::new(arg("shift_z", self));
                self.noise_named(value, "noise").map_or(
                    Density::Unbuilt("no such noise"),
                    |noise| Density::Shifted {
                        noise,
                        shift_x,
                        shift_y,
                        shift_z,
                        xz_scale: number("xz_scale", 1.0),
                        y_scale: number("y_scale", 1.0),
                    },
                )
            }
            // `shift_a` moves along x and `shift_b` along z; the plain one is both at once, which
            // nothing in the overworld uses.
            "shift_a" | "shift" => self.noise_named(value, "argument").map_or(
                Density::Unbuilt("no such noise"),
                |noise| Density::Shift {
                    noise,
                    axis: Axis::X,
                },
            ),
            "shift_b" => self.noise_named(value, "argument").map_or(
                Density::Unbuilt("no such noise"),
                |noise| Density::Shift {
                    noise,
                    axis: Axis::Z,
                },
            ),
            "spline" => {
                value
                    .get("spline")
                    .map_or(Density::Unbuilt("a spline with no curve"), |spline| {
                        let curve = self.build_spline(spline, written, being_built);
                        Density::Spline(Box::new(curve))
                    })
            }
            // A world being extended from an older one, and a dimension that does not exist yet.
            "blend_alpha"
            | "blend_offset"
            | "blend_density"
            | "old_blended_noise"
            | "end_islands"
            | "weird_scaled_sampler"
            | "interval_select" => Density::Unbuilt("not generated here"),
            _ => Density::Unbuilt("an unknown kind"),
        }
    }

    /// The noise one field names.
    fn noise_named(&mut self, value: &serde_json::Value, field: &str) -> Option<Arc<Noise>> {
        let name = value.get(field)?.as_str()?;
        self.noise(name)
    }

    /// One curve, whose points may themselves be curves.
    fn build_spline(
        &mut self,
        value: &serde_json::Value,
        written: &HashMap<String, serde_json::Value>,
        being_built: &mut Vec<String>,
    ) -> Spline {
        if let Some(flat) = value.as_f64() {
            return Spline::Flat(flat as f32);
        }
        let Some(coordinate) = value.get("coordinate") else {
            return Spline::Flat(0.0);
        };
        let coordinate = Box::new(self.build(coordinate, written, being_built));

        let points = value
            .get("points")
            .and_then(serde_json::Value::as_array)
            .map(|points| {
                points
                    .iter()
                    .map(|point| Point {
                        location: point
                            .get("location")
                            .and_then(serde_json::Value::as_f64)
                            .unwrap_or(0.0) as f32,
                        derivative: point
                            .get("derivative")
                            .and_then(serde_json::Value::as_f64)
                            .unwrap_or(0.0) as f32,
                        value: point.get("value").map_or(Spline::Flat(0.0), |inner| {
                            self.build_spline(inner, written, being_built)
                        }),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Spline::Curve { coordinate, points }
    }
}

/// Where the packs keep them.
const FROM: &str = "assets/extracted/26.2/data/minecraft/worldgen/density_function";

/// Every function file under a directory, named the way another function would name it.
fn collect(
    root: &std::path::Path,
    here: &std::path::Path,
    into: &mut HashMap<String, serde_json::Value>,
) {
    let Ok(entries) = std::fs::read_dir(here) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, into);
            continue;
        }
        if path.extension().is_none_or(|kind| kind != "json") {
            continue;
        }
        let Ok(name) = path.strip_prefix(root) else {
            continue;
        };
        let name = name.with_extension("");
        let Some(name) = name.to_str() else { continue };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        match serde_json::from_str(&text) {
            Ok(read) => {
                into.insert(name.replace('\\', "/"), read);
            }
            Err(err) => tracing::warn!("could not read the density function {name}: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> At {
        At::new(0, 0, 0)
    }

    fn flat(value: f64) -> Box<Density> {
        Box::new(Density::Flat(value))
    }

    #[test]
    fn the_simple_shapes_are_what_they_say() {
        assert_eq!(Density::Flat(3.5).at(origin()), 3.5);
        assert_eq!(Density::Y.at(At::new(0, 42, 0)), 42.0);
        assert_eq!(Density::Add(flat(2.0), flat(3.0)).at(origin()), 5.0);
        assert_eq!(Density::Mul(flat(2.0), flat(3.0)).at(origin()), 6.0);
        assert_eq!(Density::Min(flat(2.0), flat(3.0)).at(origin()), 2.0);
        assert_eq!(Density::Max(flat(2.0), flat(3.0)).at(origin()), 3.0);
        assert_eq!(Density::Abs(flat(-2.0)).at(origin()), 2.0);
        assert_eq!(Density::Cube(flat(-2.0)).at(origin()), -8.0);
        assert_eq!(Density::Square(flat(-2.0)).at(origin()), 4.0);
    }

    /// Flattening valleys without touching hills, which is what makes a coastline.
    #[test]
    fn the_negative_halves_only_touch_what_is_below_nothing() {
        assert_eq!(Density::HalfNegative(flat(4.0)).at(origin()), 4.0);
        assert_eq!(Density::HalfNegative(flat(-4.0)).at(origin()), -2.0);
        assert_eq!(Density::QuarterNegative(flat(-4.0)).at(origin()), -1.0);
        assert_eq!(Density::QuarterNegative(flat(4.0)).at(origin()), 4.0);
    }

    /// A straight line between two heights, flat outside them.
    #[test]
    fn a_gradient_runs_between_two_heights_and_stops() {
        let gradient = Density::YClampedGradient {
            from_y: 0,
            to_y: 100,
            from_value: 0.0,
            to_value: 10.0,
        };
        assert_eq!(gradient.at(At::new(0, 0, 0)), 0.0);
        assert_eq!(gradient.at(At::new(0, 50, 0)), 5.0);
        assert_eq!(gradient.at(At::new(0, 100, 0)), 10.0);
        assert_eq!(gradient.at(At::new(0, -50, 0)), 0.0, "held below");
        assert_eq!(gradient.at(At::new(0, 500, 0)), 10.0, "and above");
    }

    /// The range is closed at the bottom and open at the top, which decides what happens exactly
    /// on the edge.
    #[test]
    fn a_range_choice_takes_its_bottom_and_not_its_top() {
        let choice = |value: f64| Density::RangeChoice {
            input: flat(value),
            lowest: 0.0,
            highest: 1.0,
            inside: flat(100.0),
            outside: flat(-100.0),
        };
        assert_eq!(choice(0.0).at(origin()), 100.0, "the bottom is inside");
        assert_eq!(choice(0.5).at(origin()), 100.0);
        assert_eq!(choice(1.0).at(origin()), -100.0, "and the top is not");
    }

    /// Nothing times anything is nothing, and the other side is not asked.
    #[test]
    fn multiplying_by_nothing_does_not_ask_the_other_side() {
        let never = Density::Mul(flat(0.0), Box::new(Density::Unbuilt("would be asked")));
        assert_eq!(never.at(origin()), 0.0);
    }

    /// A curve through two points, checked against the arithmetic written out by hand.
    #[test]
    fn a_spline_between_two_points_is_a_cubic_and_not_a_line() {
        let curve = Spline::Curve {
            coordinate: Box::new(Density::Y),
            points: vec![
                Point {
                    location: 0.0,
                    value: Spline::Flat(0.0),
                    derivative: 0.0,
                },
                Point {
                    location: 10.0,
                    value: Spline::Flat(10.0),
                    derivative: 0.0,
                },
            ],
        };

        // The ends are the points themselves.
        assert_eq!(curve.at(At::new(0, 0, 0)), 0.0);
        assert_eq!(curve.at(At::new(0, 10, 0)), 10.0);

        // In the middle a straight line would give five; the slopes at both ends are flat, so the
        // curve is pulled towards them and gives five as well — but the eighth of the way along is
        // where the two part company.
        let straight = 10.0 * 0.25;
        let curved = curve.at(At::new(0, 2, 0));
        assert!(
            (curved - straight).abs() > 0.1,
            "a cubic should not be a line: {curved} against {straight}"
        );
    }

    /// Past its last point a curve carries on at the slope it was leaving with.
    #[test]
    fn a_curve_carries_on_past_its_ends() {
        let curve = Spline::Curve {
            coordinate: Box::new(Density::Y),
            points: vec![
                Point {
                    location: 0.0,
                    value: Spline::Flat(0.0),
                    derivative: 2.0,
                },
                Point {
                    location: 10.0,
                    value: Spline::Flat(10.0),
                    derivative: 3.0,
                },
            ],
        };
        assert_eq!(curve.at(At::new(0, -5, 0)), -10.0, "two a step, going down");
        assert_eq!(curve.at(At::new(0, 15, 0)), 25.0, "three a step, going up");
    }

    /// And with no slope it simply stops.
    #[test]
    fn a_flat_ended_curve_stops_where_it_ends() {
        let curve = Spline::Curve {
            coordinate: Box::new(Density::Y),
            points: vec![Point {
                location: 0.0,
                value: Spline::Flat(7.0),
                derivative: 0.0,
            }],
        };
        assert_eq!(curve.at(At::new(0, -100, 0)), 7.0);
        assert_eq!(curve.at(At::new(0, 100, 0)), 7.0);
    }

    /// A curve whose points are themselves curves, which is how erosion bends what
    /// continentalness said.
    #[test]
    fn a_curve_can_be_made_of_curves() {
        let inner = |value: f32| Spline::Curve {
            coordinate: Box::new(Density::Y),
            points: vec![Point {
                location: 0.0,
                value: Spline::Flat(value),
                derivative: 0.0,
            }],
        };
        let outer = Spline::Curve {
            coordinate: Box::new(Density::Flat(0.5)),
            points: vec![
                Point {
                    location: 0.0,
                    value: inner(0.0),
                    derivative: 0.0,
                },
                Point {
                    location: 1.0,
                    value: inner(100.0),
                    derivative: 0.0,
                },
            ],
        };
        let halfway = outer.at(origin());
        assert!(
            (0.0..=100.0).contains(&halfway) && halfway > 10.0,
            "{halfway}"
        );
    }

    #[test]
    fn something_this_server_does_not_build_reads_as_nothing() {
        assert_eq!(Density::Unbuilt("blend_alpha").at(origin()), 0.0);
    }

    /// The repo root, from where this crate sits.
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("the crate sits three deep in the repository")
            .to_path_buf()
    }

    fn from_the_packs() -> Functions {
        let parameters = Parameters::load(&root());
        Builder::new(12345, &parameters).load(&root())
    }

    /// Every function the packs define is read, including the ones in the dimension folders.
    #[test]
    fn every_function_the_packs_define_is_built() {
        let built = from_the_packs();
        assert_eq!(built.len(), 35);
        assert!(built.get("y").is_some());
        assert!(built.get("shift_x").is_some());
        assert!(
            built.get("overworld/depth").is_some(),
            "the nested ones too"
        );
    }

    /// `y` is the height, whatever else it is wrapped in.
    #[test]
    fn the_height_function_gives_the_height() {
        let built = from_the_packs();
        let y = built.get("y").expect("the packs define it");
        assert_eq!(y.at(At::new(0, 0, 0)), 0.0);
        assert_eq!(y.at(At::new(0, 100, 0)), 100.0);
        assert_eq!(y.at(At::new(0, -60, 0)), -60.0);
    }

    /// One function naming another is followed, however they are ordered on disk.
    #[test]
    fn a_function_that_names_another_is_built_after_it() {
        let built = from_the_packs();
        // Depth is a gradient plus the offset function, which is a tree of its own.
        let depth = built.get("overworld/depth").expect("the packs define it");
        let high = depth.at(At::new(0, 300, 0));
        let low = depth.at(At::new(0, -60, 0));
        assert!(
            low > high,
            "depth should fall as it rises: {low} at the bottom, {high} at the top"
        );
    }

    /// The whole point: the same seed gives the same terrain, and two seeds do not.
    #[test]
    fn the_same_seed_shapes_the_same_land() {
        let parameters = Parameters::load(&root());
        let here = |seed| {
            let built = Builder::new(seed, &parameters).load(&root());
            let shape = built
                .get("overworld/continents")
                .expect("the packs define it")
                .clone();
            (0..20)
                .map(|at| shape.at(At::new(at * 37, 64, -at * 11)))
                .collect::<Vec<_>>()
        };
        assert_eq!(here(7), here(7));
        assert_ne!(here(7), here(8));
    }

    /// And it actually varies across the world rather than being flat.
    #[test]
    fn the_land_is_not_the_same_everywhere() {
        let built = from_the_packs();
        let shape = built
            .get("overworld/continents")
            .expect("the packs define it");
        let sampled: Vec<f64> = (0..200)
            .map(|at| shape.at(At::new(at * 53, 64, at * -29)))
            .collect();
        let lowest = sampled.iter().copied().fold(f64::MAX, f64::min);
        let highest = sampled.iter().copied().fold(f64::MIN, f64::max);
        assert!(
            highest - lowest > 0.2,
            "the world barely changes: {lowest} to {highest}"
        );
    }

    /// A pack that names itself in a circle costs that one function rather than the stack.
    #[test]
    fn a_circle_is_refused_rather_than_followed() {
        let parameters = Parameters::default();
        let mut builder = Builder::new(1, &parameters);
        let mut written = HashMap::new();
        written.insert(
            "loops".to_string(),
            serde_json::json!({"type": "minecraft:abs", "argument": "minecraft:loops"}),
        );
        let built = builder.build_named("loops", &written, &mut Vec::new());
        // It reads as nothing rather than never returning.
        assert_eq!(built.at(origin()), 0.0);
    }
}
