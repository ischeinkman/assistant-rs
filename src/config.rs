use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::utils::IterUtils;

use arpabet::{
    phoneme::Phoneme, Arpabet,
};

#[derive(PartialEq, Debug, Clone)]
pub enum PhonePart {
    Phoneme(Phoneme),
    Space,
    Unknown(String),
}

#[derive(Debug, Clone)]
struct CommandMessage {
    raw: String,
    phones: Vec<PhonePart>,
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
    pub fn message_ap(&self) -> &[PhonePart] {
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
        let phones = conv(&raw).collect();
        Ok(CommandMessage { raw, phones })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DeepspeechConfig {
    pub library_path: Option<PathBuf>,
    pub model_path: Option<PathBuf>,
    pub scorer_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    pub deepspeech_config: DeepspeechConfig,
    pub commands: Vec<Command>,
}

pub fn conv<'a>(raw_msg: &'a str) -> impl Iterator<Item = PhonePart> + 'a {
    let ap = Arpabet::load_cmudict();
    raw_msg
        .split_whitespace()
        .flat_map(move |w| {
            let nxt = match ap.get_polyphone(w) {
                Some(pw) => pw.into_iter().map(|p| PhonePart::Phoneme(p)).left(),
                None => std::iter::once(PhonePart::Unknown(w.to_owned())).right(),
            };
            std::iter::once(PhonePart::Space).chain(nxt)
        })
        .skip(1)
}

struct StringVisitor {}
impl StringVisitor {
    pub const fn new() -> Self {
        Self {}
    }
}
impl<'a> serde::de::Visitor<'a> for StringVisitor {
    type Value = String;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a string.")
    }
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v)
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v.to_owned())
    }
}

