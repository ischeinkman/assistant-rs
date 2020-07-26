use crate::error::{ConfigError, PhonemeConvertionError};
use crate::speech::Utterance;
use crate::utils::StringVisitor;
use serde::de;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Hash, Clone)]
pub struct ModeTree {
    #[serde(default)]
    #[serde(rename = "command")]
    default_mode_commands: Vec<Command>,

    #[serde(default)]
    #[serde(rename = "mode")]
    submodes: Vec<CommandMode>,
}

impl Default for ModeTree {
    fn default() -> Self {
        Self::empty()
    }
}

impl ModeTree {
    pub fn empty() -> Self {
        Self {
            default_mode_commands: Vec::new(),
            submodes: Vec::new(),
        }
    }
    pub fn with_commands(mut self, commands: Vec<Command>) -> Result<Self, ConfigError> {
        let mut commands = commands;
        let mut messages = HashSet::new();
        let mut new_default_mode = std::mem::take(&mut self.default_mode_commands);
        new_default_mode.append(&mut commands);
        for cmd in new_default_mode.iter() {
            if messages.contains(cmd.message()) {
                return Err(ConfigError::DuplicateMessage(cmd.message().to_owned()));
            } else {
                messages.insert(cmd.message());
            }
        }
        self.default_mode_commands = new_default_mode;
        Ok(self)
    }
    #[allow(unused)]
    pub fn with_mode(mut self, name: String, commands: Vec<Command>) -> Result<Self, ConfigError> {
        if self.has_mode(&name) {
            return Err(ConfigError::DuplicateMode(name));
        }
        let new_mode = CommandMode { name, commands };
        self.submodes.push(new_mode);
        Ok(self)
    }
    pub fn or_else(self, other: Self) -> Result<Self, ConfigError> {
        let mut other = other;
        let mut retvl = self.with_commands(other.default_mode_commands)?;
        let mut new_submodes = std::mem::take(&mut retvl.submodes);
        new_submodes.append(&mut other.submodes);
        let mut mode_names = HashSet::with_capacity(new_submodes.len());
        for md in new_submodes.iter() {
            if mode_names.contains(&md.name) {
                return Err(ConfigError::DuplicateMode(md.name.to_owned()));
            }
            else {
                mode_names.insert(&md.name);
            }
        } 
        retvl.submodes = new_submodes;
        Ok(retvl)
    }
    fn has_mode(&self, name: &str) -> bool {
        self.submodes.iter().any(|md| md.name == name)
    }
    pub fn commands_for_mode<'a, 'b>(
        &'a self,
        mode: Option<&'b str>,
    ) -> impl Iterator<Item = &'a Command> + 'a {
        let retvl = mode
            .and_then(|m| self.submodes.iter().find(|p| p.name == m.as_ref()))
            .map(|md| md.commands.iter())
            .unwrap_or_else(|| self.default_mode_commands.iter());
        retvl
    }
    pub fn verify(&self) -> Result<(), ConfigError> {
        if self.default_mode_commands.is_empty() {
            return Err(ConfigError::NoCommands);
        }
        let mut mode_keys: HashSet<_> = HashSet::new();
        let mut back_refs : HashSet<_> = HashSet::new();
        for md in &self.submodes {
            if md.commands.is_empty() {
                return Err(ConfigError::EmptyMode(md.name.to_owned()));
            }
            mode_keys.insert(md.name.as_str());
        }
        let cmd_iter = self
            .default_mode_commands
            .iter()
            .chain(self.submodes.iter().flat_map(|md| md.commands.iter()));
        let mode_refs = cmd_iter.filter_map(|c| c.next_mode());
        for md in mode_refs {
            if !mode_keys.contains(md) {
                return Err(ConfigError::ModeNotFound(md.to_owned()));
            }
            back_refs.insert(md);
        }
        for defined in mode_keys {
            if !back_refs.contains(defined) {
                return Err(ConfigError::UnreachableMode(defined.to_owned()));
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Hash, Clone, Default)]
struct CommandMode {
    name: String,
    #[serde(default)]
    #[serde(rename = "command")]
    commands: Vec<Command>,
}

/// A single keyphrase-activated action to run.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Command {
    message: CommandMessage,
    #[serde(default)]
    command: Option<String>,
    #[serde(rename = "mode", default)]
    next_mode: Option<String>,
}

impl Command {

    /// Constructs a new `Command`. 

    #[allow(unused)]
    pub fn new(
        message: CommandMessage,
        command: Option<String>,
        next_mode: Option<String>,
    ) -> Self {
        Self {
            message,
            command,
            next_mode,
        }
    }
    /// Returns the terminal command that will be run if the keyphrase is matched.
    pub fn command(&self) -> Option<&str> {
        self.command.as_ref().map(|s| s.as_ref())
    }

    /// Returns the next mode that the model will switch to after this command is run, if it exists.
    /// A `None` value indicates that once this command node is reached, the current voice exchange will finish.
    pub fn next_mode(&self) -> Option<&str> {
        self.next_mode.as_ref().map(|s| s.as_ref())
    }

    /// Returns the keyphrase used to run this command.
    pub fn message(&self) -> &str {
        &self.message.raw
    }
}

/// The keyphrase used to run a command.
#[derive(Debug, Clone, Serialize)]
pub struct CommandMessage {
    raw: String,

    #[serde(skip_serializing)]
    phones: Utterance,
}

impl CommandMessage {
    pub fn from_raw(raw: String) -> Result<Self, PhonemeConvertionError> {
        let phones = Utterance::parse_with_unknowns(&raw);
        Ok(Self { raw, phones })
    }
}

impl<'de> Deserialize<'de> for CommandMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = deserializer.deserialize_string(StringVisitor::new())?;
        Self::from_raw(raw).map_err(|e| {
            de::Error::invalid_value(
                de::Unexpected::Str(&e.raw),
                &"a message convertable to a list of phonemes",
            )
        })
    }
}

impl PartialEq for CommandMessage {
    fn eq(&self, other: &CommandMessage) -> bool {
        self.raw == other.raw
    }
}

impl Eq for CommandMessage {}

impl std::hash::Hash for CommandMessage {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state)
    }
}
