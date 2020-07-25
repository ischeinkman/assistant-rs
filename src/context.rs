use crate::buffer::{AudioReciever, SpeechLoader};
use crate::config;
use crate::config::{Config, DeepspeechConfig};
use crate::error::AssistantRsError;
use crate::metrics;
use crate::modes::Command;

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
        log::log!(log::Level::Debug, "Starting run.");
        while cur_mode.is_some() {
            log::log!(
                log::Level::Debug,
                "Next mode: {}",
                cur_mode.as_ref().unwrap()
            );
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
    // Get the raw transcription of the audio.
    let final_msg = get_raw_utterance(model)?;
    let final_msg = final_msg.trim();
    log::log!(log::Level::Debug, "Finished message: {}", final_msg);

    // Match the command, currently via minimum edit distance.
    let (commands, next_mode) = match_commands(&config.modes, current_mode, final_msg);
    log::log!(log::Level::Debug, "Command buff: {:?}", commands);
    log::log!(log::Level::Debug, "Returned mode: {:?}", next_mode);
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
    conf: &'a crate::modes::ModeTree,
    current_mode: Option<&str>,
    raw_text: &str,
) -> (Vec<&'a str>, Option<String>) {
    let mut mode = current_mode;
    let mut command_buff = Vec::new();
    let mut str_buff = "".to_owned();
    loop {
        // Get all the edges from this node
        let current_commands = conf.commands_for_mode(mode);

        // Tries to match the next edge from the current
        let mut matched_cmd: Option<&Command> = None;
        let mut matched_cmd_dist = crate::metrics::leven_dist(&str_buff, raw_text);
        for cur in current_commands {
            // If the message is blank, this is the "default" end command.
            // Only run it if we didn't already find a better match.
            if cur.message().trim().is_empty() {
                if matched_cmd.is_none() {
                    matched_cmd = Some(cur);
                }
                continue;
            }

            let cur_msg = format!("{} {}", str_buff, cur.message());
            let cur_dist = metrics::leven_dist(&cur_msg, raw_text);
            let is_initial_cmd = str_buff.is_empty() && matched_cmd.is_none();
            if cur_dist < matched_cmd_dist || is_initial_cmd {
                matched_cmd = Some(cur);
                matched_cmd_dist = cur_dist;
            }
        }

        // If we moved along an edge to a new node, record the next command and path component
        if let Some(cmd) = matched_cmd {
            if let Some(term_cmd) = cmd.command() {
                command_buff.push(term_cmd);
            }
            mode = cmd.next_mode();
            if mode.is_some() {
                str_buff.push(' ');
                str_buff.push_str(cmd.message());
            }
        }
        // If we did not progress or progressed to a terminal node, break
        if matched_cmd.is_none() || mode.is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::{Command, CommandMessage, ModeTree};
    #[test]
    fn test_command_match() {
        let tree = ModeTree::empty();
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
        let tree = tree.with_commands(root).unwrap();
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
        let tree = tree.with_mode("firefox".to_owned(), firefox).unwrap();
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

        let tree = tree.with_mode("youtube".to_owned(), youtube).unwrap();
        let (to_run, next_mode) = match_commands(&tree, None, "firefox youtube");
        assert_eq!(to_run, Vec::<String>::new());
        assert_eq!(next_mode.as_ref().map(|s| s.as_ref()), Some("youtube"));
    }
}
