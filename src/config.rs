use serde::{Deserialize, Serialize};
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
    pub fn message_ap(&self) -> &Utterance {
        &self.message.phones
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

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DeepspeechConfig {
    pub library_path: Option<PathBuf>,
    pub model_path: Option<PathBuf>,
    pub scorer_path: Option<PathBuf>,
    pub beam_width: Option<u16>,
}

impl DeepspeechConfig {
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

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    pub deepspeech_config: DeepspeechConfig,
    pub commands: Vec<Command>,
}

impl Config {
    pub fn verify(&self) -> Result<(), ConfigError> {
        self.deepspeech_config.verify()?;
        if self.commands.is_empty() {
            return Err(ConfigError::NoCommands);
        }
        Ok(())
    }
}
