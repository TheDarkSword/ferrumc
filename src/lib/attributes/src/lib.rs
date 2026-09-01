//! Every number an entity has that something else can change.
//!
//! Health, speed, armour, reach, how hard it hits and how far it falls before that hurts are all
//! attributes: a base value the kind of entity was born with, plus a stack of modifiers from armour,
//! effects, enchantments and commands. Nothing writes those numbers directly — a speed potion adds
//! a modifier and takes it away again, and the base is never touched, which is what makes taking it
//! away exact.
//!
//! The order the modifiers are applied in is the part worth reading twice. Everything that adds a
//! flat amount goes first and forms a new base; everything that adds a share of *that* base goes
//! next; and everything that multiplies the running total goes last. Applying them in any other
//! order gives a different answer for the same set of modifiers.

use bevy_ecs::prelude::Component;
use ferrumc_data::attributes::Attribute;
use ferrumc_data::generated::default_attributes::defaults_for;
use std::borrow::Cow;

/// How a modifier changes what an attribute is worth.
///
/// The numbers are the wire's own, so a modifier read off an item or sent to a client needs no
/// translating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Operation {
    /// Adds a flat amount to the base.
    AddValue = 0,
    /// Adds a share of the base, after every flat amount has been added to it.
    AddMultipliedBase = 1,
    /// Multiplies the running total.
    AddMultipliedTotal = 2,
}

impl Operation {
    /// The number the wire carries.
    #[must_use]
    pub const fn wire_id(self) -> i32 {
        self as i32
    }
}

/// One thing changing what an attribute is worth.
///
/// The name is what makes taking it away again possible: adding a second modifier under a name that
/// is already there replaces it rather than stacking, which is how re-equipping the same boots does
/// not double their armour.
#[derive(Debug, Clone, PartialEq)]
pub struct Modifier {
    pub name: Cow<'static, str>,
    pub amount: f64,
    pub operation: Operation,
}

impl Modifier {
    /// A modifier from something the server itself knows the name of, which is almost all of them.
    #[must_use]
    pub const fn known(name: &'static str, amount: f64, operation: Operation) -> Self {
        Self {
            name: Cow::Borrowed(name),
            amount,
            operation,
        }
    }
}

/// One attribute on one entity: what it was born with, and what has changed it since.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    /// What the kind of entity was born with, which nothing but a command changes.
    base: f64,
    modifiers: Vec<Modifier>,
    /// What it comes to, worked out when something changes rather than when it is asked for.
    value: f64,
    /// Which attribute this is, which is what says the range the value is held to.
    attribute: &'static Attribute,
}

impl Instance {
    /// What it comes to.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// What it was born with.
    #[must_use]
    pub const fn base(&self) -> f64 {
        self.base
    }

    /// Sets what it was born with, which is what a command does and nothing else should.
    pub fn set_base(&mut self, base: f64) {
        self.base = base;
        self.recalculate();
    }

    /// Adds a modifier, replacing any already there under the same name.
    ///
    /// Returns whether anything actually changed, so a caller can skip telling a client about a
    /// modifier that was already exactly there.
    pub fn add(&mut self, modifier: Modifier) -> bool {
        match self
            .modifiers
            .iter_mut()
            .find(|held| held.name == modifier.name)
        {
            Some(held) if *held == modifier => return false,
            Some(held) => *held = modifier,
            None => self.modifiers.push(modifier),
        }
        self.recalculate();
        true
    }

    /// Takes a modifier away by name.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.modifiers.len();
        self.modifiers.retain(|held| held.name != name);
        if self.modifiers.len() == before {
            return false;
        }
        self.recalculate();
        true
    }

    /// Takes away every modifier whose name starts with something.
    pub fn remove_by_prefix(&mut self, prefix: &str) -> bool {
        let before = self.modifiers.len();
        self.modifiers.retain(|held| !held.name.starts_with(prefix));
        if self.modifiers.len() == before {
            return false;
        }
        self.recalculate();
        true
    }

    /// Whatever is changing it.
    pub fn modifiers(&self) -> impl Iterator<Item = &Modifier> {
        self.modifiers.iter()
    }

    /// Works out what it comes to, in the one order that is right.
    fn recalculate(&mut self) {
        let mut base = self.base;
        for modifier in &self.modifiers {
            if modifier.operation == Operation::AddValue {
                base += modifier.amount;
            }
        }

        // A share of the base *after* the flat amounts, not of what the entity was born with. Two
        // pieces of armour each adding a tenth add a tenth of the same number, not of each other's.
        let mut total = base;
        for modifier in &self.modifiers {
            if modifier.operation == Operation::AddMultipliedBase {
                total += base * modifier.amount;
            }
        }
        for modifier in &self.modifiers {
            if modifier.operation == Operation::AddMultipliedTotal {
                total *= 1.0 + modifier.amount;
            }
        }

        self.value = self.attribute.clamp(total);
    }
}

/// Every attribute one entity has.
///
/// Kept as a flat list rather than a map: a mob has under forty of them and most have under thirty,
/// so a walk over a contiguous run of numbers beats hashing. The list is sorted by the attribute's
/// own number, which is also the number the wire uses.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Attributes {
    held: Vec<(u16, Instance)>,
}

impl Attributes {
    /// What a kind of entity is born with.
    ///
    /// A kind with none — an arrow, a boat, a dropped item — gets an empty set rather than an
    /// error: not living is a real answer.
    #[must_use]
    pub fn for_entity(entity_type: u16) -> Self {
        let held = defaults_for(entity_type)
            .iter()
            .filter_map(|(id, base)| {
                let attribute = Attribute::from_id(*id)?;
                Some((
                    *id,
                    Instance {
                        base: *base,
                        modifiers: Vec::new(),
                        value: attribute.clamp(*base),
                        attribute,
                    },
                ))
            })
            .collect();
        Self { held }
    }

    /// Whether it has any at all, which is to say whether it lives.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// What one attribute comes to, or what everything is born with where this entity has no say.
    #[must_use]
    pub fn value(&self, attribute: &'static Attribute) -> f64 {
        self.get(attribute)
            .map_or(attribute.default_value, Instance::value)
    }

    /// One attribute, where the entity has it.
    #[must_use]
    pub fn get(&self, attribute: &'static Attribute) -> Option<&Instance> {
        self.held
            .binary_search_by_key(&attribute.id, |(id, _)| *id)
            .ok()
            .map(|at| &self.held[at].1)
    }

    /// One attribute to change, where the entity has it.
    pub fn get_mut(&mut self, attribute: &'static Attribute) -> Option<&mut Instance> {
        let at = self
            .held
            .binary_search_by_key(&attribute.id, |(id, _)| *id)
            .ok()?;
        Some(&mut self.held[at].1)
    }

    /// Adds a modifier to one attribute, if the entity has that attribute at all.
    pub fn add(&mut self, attribute: &'static Attribute, modifier: Modifier) -> bool {
        self.get_mut(attribute)
            .is_some_and(|instance| instance.add(modifier))
    }

    /// Takes a modifier away from one attribute.
    pub fn remove(&mut self, attribute: &'static Attribute, name: &str) -> bool {
        self.get_mut(attribute)
            .is_some_and(|instance| instance.remove(name))
    }

    /// Takes away every modifier whose name starts with something.
    ///
    /// One slot's worth: emptying a slot has to drop whatever it used to add, without needing to
    /// remember which attributes that was.
    pub fn remove_by_prefix(&mut self, prefix: &str) -> bool {
        let mut changed = false;
        for (_, instance) in &mut self.held {
            changed |= instance.remove_by_prefix(prefix);
        }
        changed
    }

    /// Takes a modifier away from every attribute that carries it.
    ///
    /// What one piece of armour changes is several attributes at once, and taking it off should not
    /// need to remember which.
    pub fn remove_everywhere(&mut self, name: &str) -> bool {
        let mut changed = false;
        for (_, instance) in &mut self.held {
            changed |= instance.remove(name);
        }
        changed
    }

    /// Everything the entity has, in the order the wire wants it.
    pub fn iter(&self) -> impl Iterator<Item = (&'static Attribute, &Instance)> {
        self.held
            .iter()
            .map(|(_, instance)| (instance.attribute, instance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers the kinds travel as, so a test reads as what it is about.
    fn kind(name: &str) -> u16 {
        let registries: serde_json::Value =
            serde_json::from_str(include_str!("../../../../assets/data/registries.json"))
                .expect("the registries are valid json");
        registries["minecraft:entity_type"]["entries"][format!("minecraft:{name}")]["protocol_id"]
            .as_u64()
            .and_then(|id| u16::try_from(id).ok())
            .expect("every kind has a number")
    }

    fn zombie() -> Attributes {
        Attributes::for_entity(kind("zombie"))
    }

    #[test]
    fn a_kind_is_born_with_what_the_game_says() {
        let zombie = zombie();
        assert_eq!(zombie.value(&Attribute::MAX_HEALTH), 20.0);
        assert_eq!(zombie.value(&Attribute::ARMOR), 2.0);
        assert_eq!(zombie.value(&Attribute::ATTACK_DAMAGE), 3.0);
        assert!((zombie.value(&Attribute::MOVEMENT_SPEED) - 0.23).abs() < 1e-6);
    }

    #[test]
    fn a_player_is_born_with_a_fist_rather_than_a_weapon() {
        let player = Attributes::for_entity(kind("player"));
        assert_eq!(player.value(&Attribute::ATTACK_DAMAGE), 1.0);
        assert_eq!(player.value(&Attribute::ATTACK_SPEED), 4.0);
        assert_eq!(player.value(&Attribute::MAX_HEALTH), 20.0);
    }

    #[test]
    fn nothing_that_does_not_live_has_any() {
        let arrow = Attributes::for_entity(kind("arrow"));
        assert!(arrow.is_empty());
        // And asking anyway gives what everything is born with, not nonsense.
        assert_eq!(
            arrow.value(&Attribute::MAX_HEALTH),
            Attribute::MAX_HEALTH.default_value
        );
    }

    #[test]
    fn a_flat_modifier_adds_to_the_base() {
        let mut zombie = zombie();
        zombie.add(
            &Attribute::ARMOR,
            Modifier::known("a helmet", 2.0, Operation::AddValue),
        );
        assert_eq!(zombie.value(&Attribute::ARMOR), 4.0);
    }

    #[test]
    fn taking_a_modifier_away_puts_the_value_back_exactly() {
        let mut zombie = zombie();
        let before = zombie.value(&Attribute::MOVEMENT_SPEED);
        zombie.add(
            &Attribute::MOVEMENT_SPEED,
            Modifier::known("speed", 0.2, Operation::AddMultipliedTotal),
        );
        assert!(zombie.value(&Attribute::MOVEMENT_SPEED) > before);

        zombie.remove(&Attribute::MOVEMENT_SPEED, "speed");
        assert_eq!(
            zombie.value(&Attribute::MOVEMENT_SPEED),
            before,
            "exactly, not nearly: the base was never touched"
        );
    }

    #[test]
    fn a_share_of_the_base_is_a_share_of_the_base_after_the_flat_amounts() {
        // Base 2, plus a flat 2, makes 4; a tenth of that is 0.4, so 4.4. Not a tenth of the
        // original 2, and not compounded with the other modifier.
        let mut zombie = zombie();
        zombie.add(
            &Attribute::ARMOR,
            Modifier::known("flat", 2.0, Operation::AddValue),
        );
        zombie.add(
            &Attribute::ARMOR,
            Modifier::known("share", 0.1, Operation::AddMultipliedBase),
        );
        assert!((zombie.value(&Attribute::ARMOR) - 4.4).abs() < 1e-9);
    }

    #[test]
    fn two_shares_of_the_base_do_not_compound_and_two_of_the_total_do() {
        let mut base = zombie();
        base.add(
            &Attribute::ARMOR,
            Modifier::known("one", 1.0, Operation::AddMultipliedBase),
        );
        base.add(
            &Attribute::ARMOR,
            Modifier::known("two", 1.0, Operation::AddMultipliedBase),
        );
        assert!(
            (base.value(&Attribute::ARMOR) - 6.0).abs() < 1e-9,
            "2 + 2 + 2, not 2 * 2 * 2"
        );

        let mut total = zombie();
        total.add(
            &Attribute::ARMOR,
            Modifier::known("one", 1.0, Operation::AddMultipliedTotal),
        );
        total.add(
            &Attribute::ARMOR,
            Modifier::known("two", 1.0, Operation::AddMultipliedTotal),
        );
        assert!(
            (total.value(&Attribute::ARMOR) - 8.0).abs() < 1e-9,
            "2 * 2 * 2"
        );
    }

    #[test]
    fn the_same_name_replaces_rather_than_stacks() {
        // Which is what stops re-equipping the same boots doubling their armour.
        let mut zombie = zombie();
        zombie.add(
            &Attribute::ARMOR,
            Modifier::known("boots", 2.0, Operation::AddValue),
        );
        zombie.add(
            &Attribute::ARMOR,
            Modifier::known("boots", 2.0, Operation::AddValue),
        );
        assert_eq!(zombie.value(&Attribute::ARMOR), 4.0);
    }

    #[test]
    fn a_value_is_held_to_what_the_attribute_allows() {
        let mut zombie = zombie();
        zombie.add(
            &Attribute::ARMOR,
            Modifier::known("silly", 1000.0, Operation::AddValue),
        );
        assert_eq!(
            zombie.value(&Attribute::ARMOR),
            Attribute::ARMOR.highest,
            "armour stops at thirty however much is piled on"
        );
    }

    #[test]
    fn one_piece_of_armour_is_taken_off_everywhere_at_once() {
        let mut zombie = zombie();
        zombie.add(
            &Attribute::ARMOR,
            Modifier::known("boots", 2.0, Operation::AddValue),
        );
        zombie.add(
            &Attribute::ARMOR_TOUGHNESS,
            Modifier::known("boots", 1.0, Operation::AddValue),
        );

        assert!(zombie.remove_everywhere("boots"));
        assert_eq!(zombie.value(&Attribute::ARMOR), 2.0);
        assert_eq!(zombie.value(&Attribute::ARMOR_TOUGHNESS), 0.0);
    }

    #[test]
    fn an_attribute_a_kind_does_not_have_takes_no_modifier() {
        // A zombie has no attack speed, so nothing can change one.
        let mut zombie = zombie();
        assert!(!zombie.add(
            &Attribute::ATTACK_SPEED,
            Modifier::known("nothing", 1.0, Operation::AddValue),
        ));
    }
}
