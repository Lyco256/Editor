//! Feature and command registration without concrete service ownership.

use std::collections::BTreeMap;

use editor_types::CommandId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRegistration {
    pub id: CommandId,
    pub title: String,
    pub enabled: bool,
}

#[derive(Debug, Default)]
pub struct Registry {
    commands: BTreeMap<CommandId, CommandRegistration>,
}

impl Registry {
    pub fn register(&mut self, command: CommandRegistration) -> Option<CommandRegistration> {
        self.commands.insert(command.id.clone(), command)
    }

    pub fn commands(&self) -> impl Iterator<Item = &CommandRegistration> {
        self.commands.values()
    }
}
