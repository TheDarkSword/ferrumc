use bevy_ecs::prelude::Entity;
use ferrumc_core::identity::entity_identity::EntityIdentity;
use ferrumc_core::identity::player_identity::PlayerIdentity;

pub(crate) fn resolve_any_entity(
    iter: impl Iterator<Item = (Entity, Option<&EntityIdentity>, Option<&PlayerIdentity>)>,
) -> Vec<Entity> {
    // An ECS entity carrying neither identity is not a game entity, so `@e` must skip it.
    // Bevy stores resources as entities, and they would otherwise be selectable.
    iter.filter(|(_, entity_identity, player_identity)| {
        entity_identity.is_some() || player_identity.is_some()
    })
    .map(|(entity, _, _)| entity)
    .collect()
}
