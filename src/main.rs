mod buffer;
mod config;
mod error;
mod metrics;
mod utils;

use buffer::{SpeechLoader, WaitableBuffer};
use config::Config;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use deepspeech::dynamic::Model;
use std::sync::Arc;

fn main() {
    let raw = include_str!("../res/config.toml");
    let conf: Config = toml::from_str(raw).unwrap();
    let mut model = build_model(&conf).unwrap();
    let raw = model.get_sample_rate();

    let h = cpal::default_host();
    let mic = h.default_input_device().unwrap();
    let strmconf = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(raw as u32),
    };
    let buf = Arc::new(WaitableBuffer::new());
    let b2 = Arc::clone(&buf);
    let stream = mic
        .build_input_stream(
            &strmconf,
            move |dt: &[i16], _cb| {
                b2.push_slice(dt);
            },
            |e| Result::<(), _>::Err(e).unwrap(),
        )
        .unwrap();
    stream.play().unwrap();
    
    let strm = model.create_stream().unwrap();
    let mut loader = SpeechLoader::new(strm, raw as u32);
    loop {
        let lock = buf.wait_until(raw as usize);
        if loader.push(&lock).unwrap() {
            let msg = loader.current_text();
            println!(
                "{}, {} => {}",
                lock.len(),
                (lock.len() * 1000) / (raw as usize),
                msg
            );
        } else if loader.time_since_change() > std::time::Duration::from_millis(100)
            && !loader.current_text().is_empty()
        {
            println!("======== LOADER DEINIT ==============");
            let final_msg = loader.current_text().trim();
            let phones: Vec<_> = config::conv(final_msg).collect();
            println!("-> Final text: {}", final_msg);
            println!("-> Final phones: {:?}", phones);
            let (cmd, d) = conf
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
            println!("======== LOADER INIT ==============");
            let strm = model.create_stream().unwrap();
            loader = SpeechLoader::new(strm, raw as u32);
        }
    }
}

fn build_model(conf: &Config) -> Result<Model, error::AssistantRsError> {
    let lib = conf
        .deepspeech_config
        .library_path
        .clone()
        .unwrap_or_else(|| {
            let mut p = std::path::PathBuf::new();
            p.set_file_name("libdeepspeech.so");
            p
        });
    let model = conf
        .deepspeech_config
        .model_path
        .as_ref()
        .ok_or(error::ConfigError::NoModel)?;
    let mut retvl = Model::load_from_files(lib.as_ref(), model.as_ref())?;
    if let Some(scorer) = conf.deepspeech_config.scorer_path.as_ref() {
        retvl.enable_external_scorer(scorer)?;
    }
    if let Some(w) = conf.deepspeech_config.beam_width {
        retvl.set_model_beam_width(w)?;
    }
    Ok(retvl)
}
