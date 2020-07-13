mod buffer;
mod config;
mod error;
mod metrics;
mod speech;
mod utils;

use buffer::SpeechLoader;
use config::{Command, Config};

use cpal::traits::HostTrait;
use deepspeech::dynamic::Model;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

fn main() {
    let test_path = vec![PathBuf::from(
        "/home/ilan/Projects/assistant-rs/res/config.toml",
    )];
    let mut ctx = AssistantContext::init_from_paths(test_path).unwrap();
    ctx.run().unwrap();
}

fn build_model(conf: &Config) -> Result<Model, error::AssistantRsError> {
    let lib = conf.deepspeech_config.library_path()?;
    let model = conf.deepspeech_config.model_path()?;
    let mut retvl = Model::load_from_files(lib.as_ref(), model.as_ref())?;
    if let Some(scorer) = conf.deepspeech_config.scorer_path()? {
        retvl.enable_external_scorer(scorer)?;
    }
    if let Some(w) = conf.deepspeech_config.beam_width()? {
        retvl.set_model_beam_width(w)?;
    }
    Ok(retvl)
}

pub struct AssistantContext {
    model: Model,
    config: Config,
    config_paths: Vec<PathBuf>,
}

fn cascade_configs(paths: &[impl AsRef<Path>]) -> Result<Config, error::ConfigError> {
    let mut config = Config::default();
    for pt in paths {
        let pt_conf = Config::read_file(pt)?;
        config = config.or_else(pt_conf);
    }
    Ok(config)
}

impl AssistantContext {
    pub fn init_from_paths(config_paths: Vec<PathBuf>) -> Result<Self, error::AssistantRsError> {
        let config = cascade_configs(&config_paths)?;
        config.verify()?;
        let model = build_model(&config)?;
        Ok(Self {
            model,
            config,
            config_paths,
        })
    }
    pub fn reload(&mut self) -> Result<(), error::AssistantRsError> {
        let new_conf = cascade_configs(&self.config_paths)?;
        if self.config != new_conf {
            let new_model = build_model(&new_conf)?;
            self.model = new_model;
            self.config = new_conf;
        }
        Ok(())
    }
    fn build_audio_stream(&mut self) -> Result<buffer::AudioReciever, error::AssistantRsError> {
        let sample_rate = self.model.get_sample_rate();
        let host = cpal::default_host();
        let dev = host
            .default_input_device()
            .ok_or(error::AssistantRsError::MicrophoneNotFound)?;
        let stream_conf = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(sample_rate as u32),
        };
        buffer::AudioReciever::construct(&dev, &stream_conf)
    }
    fn run_command(&self, cmd: &Command) -> Result<(), error::AssistantRsError> {
        let raw_command = cmd.command();
        process::Command::new("sh")
            .arg("-c")
            .arg(raw_command)
            .stderr(process::Stdio::null())
            .stdout(process::Stdio::null())
            .stdin(process::Stdio::null())
            .spawn()?;
        Ok(())
    }
    pub fn run(&mut self) -> Result<(), error::AssistantRsError> {
        let sample_rate = self.model.get_sample_rate();
        let audio_recv = self.build_audio_stream()?;
        let strm = self.model.create_stream()?;
        let mut loader = SpeechLoader::new(strm, sample_rate as u32);
        loop {
            if loader.time_since_change() > Duration::from_millis(100)
                && !loader.current_text().is_empty()
            {
                break;
            }
            let l = audio_recv.wait_until(sample_rate as usize)?;
            loader.push(&l)?;
        }
        println!("======== LOADER DEINIT ==============");
        let final_samples = loader.num_samples();
        let (final_msg, _) = loader.finish();
        let final_msg = final_msg.trim();
        let phones = speech::Utterance::parse_with_unknowns(final_msg);
        println!("-> Sample count: {}", final_samples);
        println!("-> Final text: {}", final_msg);
        println!("-> Final phones: {:?}", phones);
        let (cmd, d) = self
            .config
            .commands
            .iter()
            .map(|cmd| {
                let dl = metrics::leven_dist(cmd.message(), final_msg);
                println!("= = > CMD {}: {}", cmd.message(), dl);
                (cmd, dl)
            })
            .min_by(|(_, da), (_, db)| da.partial_cmp(db).unwrap())
            .unwrap();
        println!("= > Choice: {}, distance {}", cmd.message(), d);
        self.run_command(cmd)?;
        Ok(())
    }
}
