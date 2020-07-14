mod buffer;
mod config;
mod context;
mod error;
mod metrics;
mod speech;
mod utils;

use nix::sys::signal::{SigSet, Signal};
use structopt::StructOpt;

use std::path::PathBuf;

use crate::utils::{IterUtils, StringUtils};

fn main() {
    let mut waiter = SigSet::empty();
    waiter.add(Signal::SIGUSR1);
    waiter.add(Signal::SIGCONT);
    waiter.add(Signal::SIGHUP);
    let args = Args::from_args();
    let arg_confs = args.configs.into_iter();
    let paths = arg_confs
        .chain(get_xdg_config_files().into_iter())
        .collect();
    let mut ctx = crate::context::AssistantContext::init_from_paths(paths).unwrap();
    waiter.thread_set_mask().unwrap();
    loop {
        match waiter.wait() {
            Ok(Signal::SIGHUP) => ctx.reload().unwrap(),
            Ok(Signal::SIGCONT) | Ok(Signal::SIGUSR1) => ctx.run().unwrap(),
            Ok(other) => panic!("INVALID SIGNAL: {:?}", other),
            Err(e) => {
                panic!("GOT WEIRD: {:?}", e);
            }
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "AssistantRS",
    about = "A simple, configurable, and offline voice assistant."
)]
pub struct Args {
    /// Extra config files to read from.
    #[structopt(name = "config", long = "config")]
    configs: Vec<PathBuf>,
}

pub fn get_xdg_config_files() -> Vec<PathBuf> {
    let mut retvl = Vec::new();
    let xdg_config_home = xdg_home_config_dir();
    let xdg_config_dirs = xdg_sys_config_dirs();
    let xdg_files = xdg_config_home
        .into_iter()
        .chain(xdg_config_dirs)
        .map(config_root_to_toml);
    retvl.extend(xdg_files);
    retvl
}

fn config_root_to_toml(mut pt: PathBuf) -> PathBuf {
    pt.push("assistant-rs");
    pt.push("assistant.toml");
    pt
}

fn xdg_home_config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(|s| PathBuf::from(s))
        .or_else(|| {
            let home = std::env::var_os("HOME")?;
            let mut pt = PathBuf::from(home);
            pt.push(".config");
            Some(pt)
        })
}

fn xdg_sys_config_dirs() -> impl Iterator<Item = PathBuf> {
    let xdg_config_dirs_raw = match std::env::var("XDG_CONFIG_DIRS") {
        Err(std::env::VarError::NotPresent) => std::iter::once("/etc/xdg".to_string()).right(),
        Err(std::env::VarError::NotUnicode(_raw)) => {
            todo!();
        }
        Ok(s) => s.split_owned(":").left(),
    };
    let xdg_config_dirs = xdg_config_dirs_raw.map(|s| PathBuf::from(s));
    xdg_config_dirs
}
