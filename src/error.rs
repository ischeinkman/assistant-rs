use deepspeech::errors::DeepspeechError;
use thiserror::Error;

use toml::de::Error as TomlError;

#[derive(Error, Debug)]
pub enum AssistantRsError {
    #[error("DeepSpeech library error")]
    Deepspeech(#[from] DeepspeechError),

    #[error("config error")]
    Config(#[from] ConfigError),

    #[error("no microphone found")]
    MicrophoneNotFound,

    #[error("CPAL error")]
    Cpal(#[from] CpalError), 

    #[error("Error running command")]
    RunError (#[from] std::io::Error)
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

    #[error("no path passed to --config flag")]
    NoFlagArgument
}

#[derive(Error, Debug)]
#[error("cannot convert {raw} to phoneme list")]
pub struct PhonemeConvertionError {
    pub raw: String,
}


#[derive(Error, Debug)]
pub enum CpalError {
    #[error("error building stream")]
    BuildStream(#[from] cpal::BuildStreamError), 
    #[error("error playing stream")]
    PlayStream(#[from] cpal::PlayStreamError),

    #[error("error in running stream")]
    Stream(#[from] cpal::StreamError)
}