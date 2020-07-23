use crate::error::{PhonemeConvertionError, ConfigError};
use crate::speech::Utterance;
use crate::utils::StringVisitor;
use serde::{Deserialize, Serialize, Deserializer};
use serde::de;
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Hash, Clone, Default)]
pub struct ModeTree {
    #[serde(default)]
    #[serde(rename = "command")]
    default_mode_commands: Vec<Command>,

    #[serde(default)]
    #[serde(rename = "mode")]
    submodes: Vec<CommandMode>,
}

impl ModeTree {
    pub fn or_else(self, other: Self) -> Result<Self, ConfigError> {
        let mut default_mode_commands = self.default_mode_commands;
        let current_messages: HashSet<_> = default_mode_commands
            .iter()
            .map(|c| c.message().to_owned())
            .collect();
        for root_cmd in other.default_mode_commands.into_iter() {
            if current_messages.contains(root_cmd.message()) {
                return Err(ConfigError::DuplicateMessage(root_cmd.message().to_owned()));
            } else {
                default_mode_commands.push(root_cmd);
            }
        }
        let mut submodes = self.submodes;
        let current_modes: HashSet<_> = submodes.iter().map(|m| m.name.to_owned()).collect();
        for other_mode in other.submodes.into_iter() {
            if current_modes.contains(&other_mode.name) {
                return Err(ConfigError::DuplicateMode(other_mode.name.to_owned()));
            } else {
                submodes.push(other_mode);
            }
        }
        Ok(Self {
            default_mode_commands,
            submodes,
        })
    }
    pub fn commands_for_mode(&self, mode: Option<impl AsRef<str>>) -> Option<&[Command]> {
        match mode {
            Some(m) => self
                .submodes
                .iter()
                .find(|p| p.name == m.as_ref())
                .map(|p| p.commands.as_ref()),
            None => Some(self.default_mode_commands.as_ref()),
        }
    }
    pub fn verify(&self) -> Result<(), ConfigError> {
        if self.default_mode_commands.is_empty() {
            return Err(ConfigError::NoCommands);
        }
        let mut mode_keys: HashSet<_> = HashSet::new();
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
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Hash, Clone, Default)]
pub struct CommandMode {
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
        let phones = Utterance::parse(&raw)?;
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
