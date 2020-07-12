use deepspeech::errors::DeepspeechError;
use thiserror::Error;

use toml::de::Error as TomlError;

#[derive(Error, Debug)]
pub enum AssistantRsError {
    #[error("DeepSpeech library error")]
    Deepspeech(#[from] DeepspeechError),

    #[error("config error")]
    Config(#[from] ConfigError),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("no model path passed")]
    NoModel,
    #[error("no commands passed")]
    NoCommands,
    #[error("cannot construct pronounciation for message")]
    UnprounounceableMessage(#[from] PhonemeConvertionError),
    #[error("error parsing config file")]
    Parsing(#[from] TomlError),
    #[error("error reading config file")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
#[error("cannot convert {raw} to phoneme list")]
pub struct PhonemeConvertionError {
    pub raw: String,
}
