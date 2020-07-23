use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::ConfigError;
use crate::modes::ModeTree;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct DeepspeechConfig {
    pub library_path: Option<PathBuf>,
    pub model_path: Option<PathBuf>,
    pub scorer_path: Option<PathBuf>,
    pub beam_width: Option<u16>,
}

impl DeepspeechConfig {
    /// Combines the information in `self` with `other`.
    ///
    /// If a field is defined in both `self` and `other`, the value in `self` is used.
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

    /// Returns the path to the `libdeepspeech.so` shared object file to use.
    ///
    /// Defaults to `libdeepspeech.so`, which will use the default system loader to find the shared object file.
    pub fn library_path(&self) -> Result<&Path, ConfigError> {
        if let Some(pt) = self.library_path.as_ref() {
            Ok(pt.as_ref())
        } else {
            Ok("libdeepspeech.so".as_ref())
        }
    }

    /// Returns the path to the DeepSpeech model to use.
    /// Errors if no `model_path` has been set yet.
    pub fn model_path(&self) -> Result<&Path, ConfigError> {
        if let Some(pt) = self.model_path.as_ref() {
            Ok(pt.as_ref())
        } else {
            Err(ConfigError::NoModel)
        }
    }

    /// Returns the `scorer_path` to use if it has been set, else `None`.
    pub fn scorer_path(&self) -> Result<Option<&Path>, ConfigError> {
        if let Some(pt) = self.scorer_path.as_ref() {
            Ok(Some(pt.as_ref()))
        } else {
            Ok(None)
        }
    }

    /// Returns the `beam_width` to use if it has been set, else `None`.
    pub fn beam_width(&self) -> Result<Option<u16>, ConfigError> {
        if let Some(bw) = self.beam_width {
            Ok(Some(bw))
        } else {
            Ok(None)
        }
    }

    /// Verifies that the config is complete and valid.
    pub fn verify(&self) -> Result<(), ConfigError> {
        if self.model_path.is_none() {
            return Err(ConfigError::NoModel);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(flatten)]
    pub deepspeech_config: DeepspeechConfig,

    #[serde(flatten)]
    pub modes: ModeTree,
}

impl Config {
    /// Reads configuration information from a file.
    pub fn read_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let mut fh = File::open(path)?;
        let mut raw = String::new();
        fh.read_to_string(&mut raw)?;
        let res = toml::from_str(&raw)?;
        Ok(res)
    }

    /// Verifies that the config is complete and valid.
    pub fn verify(&self) -> Result<(), ConfigError> {
        self.deepspeech_config.verify()?;
        self.modes.verify()?;
        Ok(())
    }

    /// Combines the information in `self` with `other`.
    ///
    /// If a field is defined in both `self` and `other`, the value in `self` is used.
    /// If two commands share the same message, the one in `self` is used.
    pub fn or_else(mut self, other: Config) -> Result<Self, ConfigError> {
        self.deepspeech_config = self.deepspeech_config.or_else(other.deepspeech_config);
        self.modes = self.modes.or_else(other.modes)?;
        Ok(self)
    }
}

/// Combines the configuration information in a series of files into a single `Config`.
///
/// If a file does not exist, it is silently skipped. Files are read in order, so if two
/// files specify the same field the earlier value will be used.
pub fn cascade_configs(paths: &[impl AsRef<Path>]) -> Result<Config, ConfigError> {
    let mut config = Config::default();
    for pt in paths {
        if !pt.as_ref().is_file() {
            continue;
        }
        let pt_conf = Config::read_file(pt)?;
        config = config.or_else(pt_conf)?;
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testa() {
        let raw = include_str!("../res/config.toml");
        let _cnf: Config = toml::from_str(raw).unwrap();
    }
}
