use bevy_ecs::prelude::MessageWriter;
use ferrumc_commands::{
    arg::{primitive::PrimitiveArgument, utils::parser_error, CommandArgument, ParserResult},
    CommandContext, Sender, Suggestion,
};
use ferrumc_entities::entity_type::EntityType;
use ferrumc_macros::command;
use ferrumc_messages::SpawnEntityCommand;
use ferrumc_text::TextComponent;

/// An entity type as a command reads one. The type knows its own name, so there is no table here.
#[derive(Debug, Clone, Copy)]
struct EntityTypeArg(EntityType);

impl CommandArgument for EntityTypeArg {
    fn parse(ctx: &mut CommandContext) -> ParserResult<Self> {
        let str = ctx.input.read_string();

        let Some(kind) = EntityType::from_name(&str).filter(|kind| kind.def().summon) else {
            return Err(parser_error(format!("Unknown entity type: {str}").as_str()));
        };

        Ok(EntityTypeArg(kind))
    }

    fn primitive() -> PrimitiveArgument {
        // We're parsing a single word
        PrimitiveArgument::word()
    }

    fn suggest(ctx: &mut CommandContext) -> Vec<Suggestion> {
        ctx.input.read_string();

        // Only what a command may actually make, which is what vanilla offers as well.
        EntityType::all()
            .filter(|kind| kind.def().summon)
            .map(|kind| Suggestion::of(kind.name().trim_start_matches("minecraft:")))
            .collect()
    }
}

/// Spawns an entity in front of the player.
///
/// Usage: /spawn <entity_type>
///
/// Every type the game lets a command make is accepted; the type list is the game's own.
#[command("spawn")]
fn spawn_command(
    #[sender] sender: Sender,
    #[arg] entity_type: EntityTypeArg,
    mut spawn_commands: MessageWriter<SpawnEntityCommand>,
) {
    match sender {
        Sender::Player(entity) => {
            // Write spawn command message - will be processed by spawn_command_processor system
            spawn_commands.write(SpawnEntityCommand {
                entity_type: entity_type.0,
                player_entity: entity,
            });

            sender.send_message(
                TextComponent::from(format!("{} spawned!", entity_type.0.name())),
                false,
            );
        }
        Sender::Server => {
            sender.send_message(
                TextComponent::from("Only players can use this command"),
                false,
            );
        }
    }
}
