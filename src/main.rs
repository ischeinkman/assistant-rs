mod args;
mod buffer;
mod config;
mod context;
mod error;
mod metrics;
mod modes;
mod speech;
mod utils;
use crate::args::Args;

use nix::sys::signal::{SigSet, Signal};
use structopt::StructOpt;

use simplelog::{LevelFilter, TermLogger, TerminalMode};

fn main() {
    TermLogger::init(
        LevelFilter::Debug,
        simplelog::ConfigBuilder::new().build(),
        TerminalMode::Stdout,
    )
    .unwrap();
    let args = Args::from_args();
    let mut waiter = SigSet::empty();
    let paths = args.conf_paths().collect();
    let mut ctx = crate::context::AssistantContext::init_from_paths(paths).unwrap();
    if !args.daemonize() {
        ctx.run().unwrap();
        return;
    }
    waiter.add(Signal::SIGUSR1);
    waiter.add(Signal::SIGCONT);
    waiter.add(Signal::SIGHUP);
    waiter.thread_set_mask().unwrap();
    loop {
        match waiter.wait() {
            Ok(Signal::SIGHUP) => {
                log::log!(
                    log::Level::Debug,
                    "Caught a signal to reload the assistant."
                );
                ctx.reload().unwrap();
            }
            Ok(Signal::SIGCONT) | Ok(Signal::SIGUSR1) => {
                log::log!(log::Level::Debug, "Caught a signal to run the assistant.");
                ctx.run().unwrap();
            }
            Ok(other) => panic!("INVALID SIGNAL: {:?}", other),
            Err(e) => {
                panic!("GOT WEIRD: {:?}", e);
            }
        }
    }
}
