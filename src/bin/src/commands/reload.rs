//! `/reload`: read the datapacks again.

use crate::systems::datapacks::Datapacks;
use bevy_ecs::prelude::ResMut;
use ferrumc_commands::Sender;
use ferrumc_macros::command;
use ferrumc_text::{NamedColor, TextComponentBuilder};

#[command("reload")]
fn reload(#[sender] sender: Sender, mut datapacks: ResMut<Datapacks>) {
    match datapacks.reload() {
        Ok(()) => sender.send_message(TextComponentBuilder::new("Reloading!").build(), false),
        Err(e) => sender.send_message(
            TextComponentBuilder::new(format!("Failed to reload data packs: {e}"))
                .color(NamedColor::Red)
                .build(),
            false,
        ),
    }
}
