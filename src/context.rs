use crate::buffer::{AudioReciever, SpeechLoader};
use crate::config;
use crate::config::{Config, DeepspeechConfig};
use crate::modes::Command;
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
            // Only reload the model if we need to
            if self.config.deepspeech_config != new_conf.deepspeech_config {
                let new_model = build_model(&new_conf.deepspeech_config)?;
                self.model = new_model;
            }
            self.config = new_conf;
        }
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), AssistantRsError> {
        let mut cur_mode = run_single(&mut self.model, &self.config, None)?;
        while cur_mode.is_some() {
            cur_mode = run_single(
                &mut self.model,
                &self.config,
                cur_mode.as_ref().map(|s| s.as_ref()),
            )?;
        }
        Ok(())
    }
}

/// Processes a single utterance instance to completion.
/// If this returns `Ok(None)`, then the run is complete; otherwise,
/// the run should continue using the returned `String` as
/// the name of the new root mode.
fn run_single(
    model: &mut Model,
    config: &Config,
    current_mode: Option<&str>,
) -> Result<Option<String>, AssistantRsError> {
    log::log!(log::Level::Debug, "Starting new run.");

    // Get the raw transcription of the audio.
    let final_msg = get_raw_utterance(model)?;
    let final_msg = final_msg.trim();
    log::log!(log::Level::Debug, "Finished message: {}", final_msg);

    // Match the command, currently via minimum edit distance.
    let (commands, next_mode) = match_commands(config, current_mode, final_msg);

    // Run the matched commands.
    for cmd in commands.into_iter() {
        run_command(&cmd)?;
    }

    Ok(next_mode)
}

/// Loads the DeepSpeech model from the config.
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

/// Attempts to match a raw speech string to a "path" in the mode config graph
fn match_commands<'a>(
    conf: &'a impl CommandModeStore,
    current_mode: Option<&str>,
    raw_text: &str,
) -> (Vec<&'a str>, Option<String>) {
    let mut mode = current_mode;
    let mut command_buff = Vec::new();
    let mut str_buff = "".to_owned();
    loop {
        println!("Current buff : {:?}", str_buff);
        // Get all the edges from this node
        let current_commands = conf
            .commands_for_mode((&mode).as_ref())
            .into_iter()
            .flatten();

        // Tries to match the next edge from the current
        let mut matched_cmd: Option<&Command> = None;
        for cur in current_commands {

            // The initial match is whether or not the previous buffer compounded with the current node is
            // better than just the buffer; as such, the buffer is padded with spaces to accurately measure the distance.
            let equivalent_matched_msg = match matched_cmd {
                Some(cmd) => format!("{} {}", str_buff, cmd.message()),
                None => format!("{} {}", str_buff, " ".repeat(cur.message().len())),
            };
            let matched_dist = metrics::leven_dist(&equivalent_matched_msg, raw_text);
            let cur_msg = format!("{} {}", str_buff, cur.message());
            let cur_dist = metrics::leven_dist(&cur_msg, raw_text);
            let is_initial_cmd = str_buff.is_empty() && matched_cmd.is_none();
            println!("   Test: {} => {} (vs {})", cur_msg, cur_dist, matched_dist);
            if cur_dist < matched_dist || is_initial_cmd {
                matched_cmd = Some(cur);
            }
        }

        // If we moved along an edge to a new node, record the next command and path component
        if let Some(cmd) = matched_cmd {
            if let Some(term_cmd) = cmd.command() {
                command_buff.push(term_cmd);
            }
            if let Some(nxt_mode) = cmd.next_mode() {
                mode = Some(nxt_mode);
                str_buff.push(' ');
                str_buff.push_str(&nxt_mode);
            } else {
                break;
            }
        }
        // Otherwise, return our new information
        else {
            break;
        }
    }
    (command_buff, mode.map(|s| s.to_owned()))
}

fn build_audio_stream(sample_rate: u32) -> Result<AudioReciever, AssistantRsError> {
    //TODO: Allow input configuration.
    let host = cpal::default_host();
    let dev = host
        .default_input_device()
        .ok_or(AssistantRsError::MicrophoneNotFound)?;
    let stream_conf = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(sample_rate),
    };
    AudioReciever::construct(&dev, &stream_conf)
}

fn get_raw_utterance(model: &mut Model) -> Result<String, AssistantRsError> {
    // Construct the speech loader and audio reciever.
    let sample_rate = model.get_sample_rate();
    let mut loader = SpeechLoader::new(model.create_stream()?, sample_rate as u32);
    let audio_recv = build_audio_stream(model.get_sample_rate().abs() as u32)?;

    // Listen for the command until the command is over.
    loop {
        log::log!(
            log::Level::Debug,
            "Current speech text: {}",
            loader.current_text()
        );
        let has_started = !loader.current_text().is_empty();
        let has_finished = has_started && loader.time_since_change() > Duration::from_millis(100);

        if has_finished {
            break;
        }
        let l = audio_recv.wait_until(sample_rate as usize)?;
        loader.push(&l)?;
    }

    // Get the raw transcription of the audio.
    let (final_msg, _) = loader.finish();
    Ok(final_msg)
}

fn run_command(cmd: &str) -> Result<(), AssistantRsError> {
    let raw_command = cmd;
    process::Command::new("sh")
        .arg("-c")
        .arg(raw_command)
        .stderr(process::Stdio::null())
        .stdout(process::Stdio::null())
        .stdin(process::Stdio::null())
        .spawn()?;
    Ok(())
}

trait CommandModeStore {
    fn commands_for_mode(&self, mode: Option<impl AsRef<str>>) -> Option<&[Command]>;
}

impl CommandModeStore for Config {
    fn commands_for_mode(&self, mode: Option<impl AsRef<str>>) -> Option<&[Command]> {
        Config::commands_for_mode(self, mode)
    }
}

impl CommandModeStore for (Vec<Command>, Vec<(String, Vec<Command>)>) {
    fn commands_for_mode(&self, mode: Option<impl AsRef<str>>) -> Option<&[Command]> {
        match mode {
            Some(m) => self
                .1
                .iter()
                .find(|(cur, _)| cur == m.as_ref())
                .map(|(_, r)| r.as_ref()),
            None => Some(self.0.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::{Command, CommandMessage};
    #[test]
    fn test_command_match() {
        let root = vec![
            Command::new(
                CommandMessage::from_raw("fire fox".to_owned()).unwrap(),
                None,
                Some("firefox".to_owned()),
            ),
            Command::new(
                CommandMessage::from_raw("telegram".to_owned()).unwrap(),
                Some("flatpak run org.telegram.telegram".to_owned()),
                None,
            ),
        ];
        let firefox = vec![
            Command::new(
                CommandMessage::from_raw("you tube".to_owned()).unwrap(),
                None,
                Some("youtube".to_owned()),
            ),
            Command::new(
                CommandMessage::from_raw("new window".to_owned()).unwrap(),
                Some("firefox".to_owned()),
                None,
            ),
            Command::new(
                CommandMessage::from_raw("blue tube".to_owned()).unwrap(),
                Some("firefox bluetoob.com".to_owned()),
                None,
            ),
        ];
        let youtube = vec![
            Command::new(
                CommandMessage::from_raw("f one".to_owned()).unwrap(),
                Some("firefox youtube.com/channel/f1".to_owned()),
                None,
            ),
            Command::new(
                CommandMessage::from_raw("f two".to_owned()).unwrap(),
                Some("firefox youtube.com/channel/f2".to_owned()),
                None,
            ),
        ];

        let tree = (
            root,
            vec![
                ("firefox".to_owned(), firefox),
                ("youtube".to_owned(), youtube),
            ],
        );
        let (to_run, next_mode) = match_commands(&tree, None, "firefox youtube");
        assert_eq!(to_run, Vec::<String>::new());
        assert_eq!(next_mode.as_ref().map(|s| s.as_ref()), Some("youtube"));
    }
}