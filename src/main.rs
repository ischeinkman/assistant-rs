mod buffer;
mod config;
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
    let mut model = Model::load_from_files(
        conf.deepspeech_config.library_path.as_ref().unwrap(),
        conf.deepspeech_config.model_path.as_ref().unwrap(),
    )
    .unwrap();
    let raw = model.get_sample_rate();
    let strm = model.create_stream().unwrap();
    let mut loader = SpeechLoader::new(strm, raw as u32);
    model.set_model_beam_width(1).unwrap();

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
