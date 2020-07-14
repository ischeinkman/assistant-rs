use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::ConfigError;
use crate::utils::StringVisitor;

use crate::speech::Utterance;

#[derive(Debug, Clone)]
struct CommandMessage {
    raw: String,
    phones: Utterance,
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

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Command {
    #[serde(
        serialize_with = "Command::serialize_message",
        deserialize_with = "Command::deserialize_message"
    )]
    message: CommandMessage,
    command: String,
}

impl Command {
    pub fn command(&self) -> &str {
        &self.command
    }
    pub fn message(&self) -> &str {
        &self.message.raw
    }
    fn serialize_message<S: serde::Serializer>(
        msg: &CommandMessage,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&msg.raw)
    }

    fn deserialize_message<'de, S: serde::Deserializer<'de>>(
        deserializer: S,
    ) -> Result<CommandMessage, S::Error> {
        let raw = deserializer.deserialize_string(StringVisitor::new())?;
        let phones = Utterance::parse(&raw).map_err(|e| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&e.raw),
                &"a message convertable to a list of phonemes",
            )
        })?;
        Ok(CommandMessage { raw, phones })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct DeepspeechConfig {
    pub library_path: Option<PathBuf>,
    pub model_path: Option<PathBuf>,
    pub scorer_path: Option<PathBuf>,
    pub beam_width: Option<u16>,
}

impl DeepspeechConfig {
    pub fn or_else(mut self, other: DeepspeechConfig) -> Self {
        if self.library_path.is_none() {
            self.library_path = other.library_path;
        }
        if self.model_path.is_none() {
            self.model_path = other.model_path;
        }
        if self.scorer_path.is_none() {
            self.scorer_path = other.scorer_path;
        }
        if self.beam_width.is_none() {
            self.beam_width = other.beam_width;
        }
        self
    }
    pub fn library_path(&self) -> Result<&Path, ConfigError> {
        if let Some(pt) = self.library_path.as_ref() {
            Ok(pt.as_ref())
        } else {
            Ok("libdeepspeech.so".as_ref())
        }
    }
    pub fn model_path(&self) -> Result<&Path, ConfigError> {
        if let Some(pt) = self.model_path.as_ref() {
            Ok(pt.as_ref())
        } else {
            Err(ConfigError::NoModel)
        }
    }
    pub fn scorer_path(&self) -> Result<Option<&Path>, ConfigError> {
        if let Some(pt) = self.scorer_path.as_ref() {
            Ok(Some(pt.as_ref()))
        } else {
            Ok(None)
        }
    }
    pub fn beam_width(&self) -> Result<Option<u16>, ConfigError> {
        if let Some(bw) = self.beam_width {
            Ok(Some(bw))
        } else {
            Ok(None)
        }
    }
    pub fn verify(&self) -> Result<(), ConfigError> {
        if self.model_path.is_none() {
            return Err(ConfigError::NoModel);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(flatten)]
    pub deepspeech_config: DeepspeechConfig,
    pub commands: Vec<Command>,
}

impl Config {
    pub fn read_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let mut fh = File::open(path)?;
        let mut raw = String::new();
        fh.read_to_string(&mut raw)?;
        let res = toml::from_str(&raw)?;
        Ok(res)
    }
    pub fn verify(&self) -> Result<(), ConfigError> {
        self.deepspeech_config.verify()?;
        if self.commands.is_empty() {
            return Err(ConfigError::NoCommands);
        }
        Ok(())
    }

    pub fn or_else(mut self, other: Config) -> Self {
        self.deepspeech_config = self.deepspeech_config.or_else(other.deepspeech_config);
        let existing_messages = self
            .commands
            .iter()
            .map(|c| &c.message.raw)
            .collect::<HashSet<_>>();
        let mut new_commands = other
            .commands
            .into_iter()
            .filter(|cmd| !existing_messages.contains(&cmd.message.raw))
            .collect::<Vec<_>>();
        self.commands.append(&mut new_commands);
        self
    }
}

pub fn cascade_configs(paths: &[impl AsRef<Path>]) -> Result<Config, ConfigError> {
    let mut config = Config::default();
    for pt in paths {
        if !pt.as_ref().is_file() {
            continue;
        }
        let pt_conf = Config::read_file(pt)?;
        config = config.or_else(pt_conf);
    }
    Ok(config)
}
