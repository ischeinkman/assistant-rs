use crate::buffer::{AudioReciever, SpeechLoader};
use crate::config;
use crate::config::{Command, Config, DeepspeechConfig};
use crate::error::AssistantRsError;
use crate::metrics;

use cpal::traits::HostTrait;
use deepspeech::dynamic::Model;

use std::path::PathBuf;
use std::process;
use std::time::Duration;

pub struct AssistantContext {
    model: Model,
    config: Config,
    config_paths: Vec<PathBuf>,
}

impl AssistantContext {
    pub fn init_from_paths(config_paths: Vec<PathBuf>) -> Result<Self, AssistantRsError> {
        let config = config::cascade_configs(&config_paths)?;
        config.verify()?;
        let model = build_model(&config.deepspeech_config)?;
        Ok(Self {
            model,
            config,
            config_paths,
        })
    }
    pub fn reload(&mut self) -> Result<(), AssistantRsError> {
        let new_conf = config::cascade_configs(&self.config_paths)?;
        if self.config != new_conf {
            if self.config.deepspeech_config != new_conf.deepspeech_config {
                let new_model = build_model(&new_conf.deepspeech_config)?;
                self.model = new_model;
            }
            self.config = new_conf;
        }
        Ok(())
    }
    fn build_audio_stream(&mut self) -> Result<AudioReciever, AssistantRsError> {
        let sample_rate = self.model.get_sample_rate();
        let host = cpal::default_host();
        let dev = host
            .default_input_device()
            .ok_or(AssistantRsError::MicrophoneNotFound)?;
        let stream_conf = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(sample_rate as u32),
        };
        AudioReciever::construct(&dev, &stream_conf)
    }
    fn run_command(&self, cmd: &Command) -> Result<(), AssistantRsError> {
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
    pub fn run(&mut self) -> Result<(), AssistantRsError> {
        log::log!(log::Level::Debug, "Starting new run.");
        let sample_rate = self.model.get_sample_rate();
        let audio_recv = self.build_audio_stream()?;
        let strm = self.model.create_stream()?;
        let mut loader = SpeechLoader::new(strm, sample_rate as u32);
        loop {
            log::log!(
                log::Level::Debug,
                "Current speech text: {}",
                loader.current_text()
            );
            if loader.time_since_change() > Duration::from_millis(100)
                && !loader.current_text().is_empty()
            {
                break;
            }
            let l = audio_recv.wait_until(sample_rate as usize)?;
            loader.push(&l)?;
        }
        let final_samples = loader.num_samples();
        let (final_msg, _) = loader.finish();
        let final_msg = final_msg.trim();
        log::log!(
            log::Level::Debug,
            "Finished at {} samples. Message: {}",
            final_samples,
            final_msg
        );
        let (cmd, d) = self
            .config
            .commands
            .iter()
            .map(|cmd| {
                let dl = metrics::leven_dist(cmd.message(), final_msg);
                log::log!(
                    log::Level::Debug,
                    "    Command match score: {} => {}",
                    cmd.message(),
                    dl
                );
                (cmd, dl)
            })
            .min_by(|(_, da), (_, db)| da.partial_cmp(db).unwrap())
            .unwrap();
        log::log!(
            log::Level::Debug,
            "Matched command {} with distance {}.",
            cmd.message(),
            d
        );
        self.run_command(cmd)?;
        Ok(())
    }
}

fn build_model(conf: &DeepspeechConfig) -> Result<Model, AssistantRsError> {
    let lib = conf.library_path()?;
    let model = conf.model_path()?;
    let mut retvl = Model::load_from_files(lib.as_ref(), model.as_ref())?;
    if let Some(scorer) = conf.scorer_path()? {
        retvl.enable_external_scorer(scorer)?;
    }
    if let Some(w) = conf.beam_width()? {
        retvl.set_model_beam_width(w)?;
    }
    Ok(retvl)
}
