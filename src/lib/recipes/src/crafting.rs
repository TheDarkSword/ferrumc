//! The crafting grid, and the two ways a recipe reads it.

use crate::ingredient::Ingredient;
use ferrumc_datapack::tag::TagRegistry;
use serde_json::Value;

/// The grid as it stands, trimmed to the corner the items actually occupy.
///
/// Vanilla trims it the same way, which is what lets a two-by-two recipe be laid anywhere in a
/// three-by-three grid and still match.
#[derive(Clone, Debug, Default)]
pub struct CraftingInput {
    width: usize,
    height: usize,
    /// Row by row, `None` where the slot is empty.
    items: Vec<Option<i32>>,
}

impl CraftingInput {
    /// Trims a grid to what is in it.
    #[must_use]
    pub fn new(width: usize, height: usize, items: &[Option<i32>]) -> Self {
        let occupied = |x: usize, y: usize| items.get(x + y * width).copied().flatten().is_some();
        let mut left = width;
        let mut right = 0;
        let mut top = height;
        let mut bottom = 0;
        for y in 0..height {
            for x in 0..width {
                if occupied(x, y) {
                    left = left.min(x);
                    right = right.max(x);
                    top = top.min(y);
                    bottom = bottom.max(y);
                }
            }
        }
        if left > right || top > bottom {
            return Self::default();
        }

        let (new_width, new_height) = (right - left + 1, bottom - top + 1);
        let mut trimmed = Vec::with_capacity(new_width * new_height);
        for y in 0..new_height {
            for x in 0..new_width {
                trimmed.push(items[(x + left) + (y + top) * width]);
            }
        }
        Self {
            width: new_width,
            height: new_height,
            items: trimmed,
        }
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<i32> {
        self.items.get(x + y * self.width).copied().flatten()
    }

    /// How many slots hold something.
    #[must_use]
    pub fn filled(&self) -> usize {
        self.items.iter().flatten().count()
    }

    /// Every item in the grid, in no particular order.
    pub fn items(&self) -> impl Iterator<Item = i32> + '_ {
        self.items.iter().flatten().copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filled() == 0
    }
}

/// A shape laid out on the grid.
#[derive(Clone, Debug)]
pub struct ShapedPattern {
    width: usize,
    height: usize,
    /// Row by row, `None` where the pattern leaves the slot empty.
    ingredients: Vec<Option<Ingredient>>,
    filled: usize,
    /// Whether the shape reads the same mirrored, in which case only one way round is tried.
    symmetrical: bool,
}

impl ShapedPattern {
    /// Reads a `pattern` and its `key`, trimming the blank rows and columns off the shape.
    pub fn parse(pattern: &Value, key: &Value) -> Option<Self> {
        let rows: Vec<&str> = pattern
            .as_array()?
            .iter()
            .map(|row| row.as_str())
            .collect::<Option<_>>()?;
        let key = key.as_object()?;

        let rows = shrink(&rows);
        let width = rows.first()?.chars().count();
        let height = rows.len();
        if width == 0 || height == 0 {
            return None;
        }

        let mut ingredients = Vec::with_capacity(width * height);
        for row in &rows {
            if row.chars().count() != width {
                return None;
            }
            for symbol in row.chars() {
                if symbol == ' ' {
                    ingredients.push(None);
                } else {
                    // A symbol the key does not define is a recipe that cannot mean what it says.
                    let ingredient = Ingredient::parse(key.get(&symbol.to_string())?)?;
                    ingredients.push(Some(ingredient));
                }
            }
        }

        let filled = ingredients.iter().flatten().count();
        let symmetrical = (0..height).all(|y| {
            (0..width).all(|x| {
                let left = ingredients[x + y * width].is_some();
                let right = ingredients[(width - x - 1) + y * width].is_some();
                left == right
            })
        });
        Some(Self {
            width,
            height,
            ingredients,
            filled,
            symmetrical,
        })
    }

    /// Whether the grid holds this shape, either way round.
    #[must_use]
    pub fn matches(&self, tags: &TagRegistry, input: &CraftingInput) -> bool {
        if input.filled() != self.filled
            || input.width() != self.width
            || input.height() != self.height
        {
            return false;
        }
        // Vanilla tries the mirror first and only bothers when the shape is not its own mirror.
        (!self.symmetrical && self.matches_one_way(tags, input, true))
            || self.matches_one_way(tags, input, false)
    }

    fn matches_one_way(&self, tags: &TagRegistry, input: &CraftingInput, mirrored: bool) -> bool {
        for y in 0..self.height {
            for x in 0..self.width {
                let at = if mirrored {
                    (self.width - x - 1) + y * self.width
                } else {
                    x + y * self.width
                };
                match (&self.ingredients[at], input.get(x, y)) {
                    (Some(ingredient), Some(item)) if ingredient.matches(tags, item) => {}
                    (None, None) => {}
                    _ => return false,
                }
            }
        }
        true
    }
}

/// Trims the blank rows and columns off a written pattern, as vanilla's `shrink` does.
fn shrink<'a>(rows: &[&'a str]) -> Vec<&'a str> {
    let first_filled = |row: &str| row.find(|c| c != ' ');
    let last_filled = |row: &str| row.rfind(|c| c != ' ');

    let left = rows.iter().filter_map(|row| first_filled(row)).min();
    let right = rows.iter().filter_map(|row| last_filled(row)).max();
    let (Some(left), Some(right)) = (left, right) else {
        return Vec::new();
    };
    let top = rows
        .iter()
        .position(|row| first_filled(row).is_some())
        .unwrap_or_default();
    let bottom = rows
        .iter()
        .rposition(|row| first_filled(row).is_some())
        .unwrap_or_default();

    rows[top..=bottom]
        .iter()
        .map(|row| &row[left..=right])
        .collect()
}

/// Whether the grid holds exactly these ingredients, in any arrangement.
///
/// Every ingredient takes one slot and no slot serves two, so this is a matching rather than a
/// walk: an ingredient that could take either of two slots must leave the one another needs.
#[must_use]
pub fn matches_shapeless(
    tags: &TagRegistry,
    ingredients: &[Ingredient],
    input: &CraftingInput,
) -> bool {
    if input.filled() != ingredients.len() {
        return false;
    }
    let items: Vec<i32> = input.items().collect();
    let mut taken = vec![false; items.len()];
    assign(tags, ingredients, &items, &mut taken, 0)
}

fn assign(
    tags: &TagRegistry,
    ingredients: &[Ingredient],
    items: &[i32],
    taken: &mut [bool],
    at: usize,
) -> bool {
    let Some(ingredient) = ingredients.get(at) else {
        return true;
    };
    for (slot, item) in items.iter().enumerate() {
        if taken[slot] || !ingredient.matches(tags, *item) {
            continue;
        }
        taken[slot] = true;
        if assign(tags, ingredients, items, taken, at + 1) {
            return true;
        }
        taken[slot] = false;
    }
    false
}
