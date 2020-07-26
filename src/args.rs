use crate::utils::{IterUtils, StringUtils};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "AssistantRS",
    about = "A simple, configurable, and offline voice assistant."
)]
pub struct Args {
    /// Extra config files to read from.
    #[structopt(name = "config", long = "config")]
    configs: Vec<PathBuf>,

    /// When this flag is passed, the program sleeps continuously in the background and
    /// can be controlled via the following Unix signals:
    ///
    /// * SIGCONT | SIGUSR1 -- listen and run a single command.
    ///
    /// * SIGHUP -- reload the assistant's configuration from the config files.
    #[structopt(name = "daemonize", long = "daemonize", short = "d")]
    daemonize: bool,
}

impl Args {
    pub fn conf_paths<'a>(&'a self) -> impl Iterator<Item = PathBuf> + 'a {
        let mut filter = MyPathFilter::default();
        let mut arg_conf_iter = self.configs.iter();
        let mut xdg_conf_iter = get_xdg_config_files().into_iter();
        let cb = move || {
            while let Some(nxt) = arg_conf_iter.next() {
                if !filter.contains(nxt) {
                    filter.insert(nxt);
                    return Some(nxt.clone());
                }
            }
            while let Some(nxt) = xdg_conf_iter.next() {
                if !filter.contains(&nxt) {
                    filter.insert(&nxt);
                    return Some(nxt);
                }
            }
            None
        };
        std::iter::from_fn(cb)
    }

    pub fn daemonize(&self) -> bool {
        self.daemonize
    }
}

struct FilteredPathIter<T: AsRef<Path>, I: Iterator<Item = T>, H: BuildHasher = RandomState> {
    iter: I,
    filter: MyPathFilter<H>,
}

impl<T: AsRef<Path>, I: Iterator<Item = T>, H: BuildHasher> Iterator for FilteredPathIter<T, I, H> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(nxt) = self.iter.next() {
            if !self.filter.contains(nxt.as_ref()) {
                self.filter.insert(nxt.as_ref());
                return Some(nxt);
            }
        }
        None
    }
}

struct MyPathFilter<H: BuildHasher = RandomState> {
    hashes: Vec<u128>,
    hasher: H,
}

impl Default for MyPathFilter<RandomState> {
    fn default() -> Self {
        Self {
            hashes: Vec::default(),
            hasher: RandomState::default(),
        }
    }
}

impl<H: BuildHasher + Default, A: AsRef<Path>> std::iter::FromIterator<A> for MyPathFilter<H> {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        let init = Self {
            hashes: Vec::default(),
            hasher: H::default(),
        };
        iter.into_iter().fold(init, |acc, cur| acc.with(cur))
    }
}

impl<H: BuildHasher> MyPathFilter<H> {
    fn hash_path(&self, path: &Path) -> u128 {
        let mut hasher = self.hasher.build_hasher();
        path.hash(&mut hasher);
        let high_bits = u128::from(hasher.finish()) << 64;
        for c in path.components() {
            c.hash(&mut hasher);
        }
        let low_bits = u128::from(hasher.finish());
        let full = high_bits | low_bits;
        full
    }

    pub fn contains_or_insert(&mut self, path: impl AsRef<Path>) -> bool {
        let hash = self.hash_path(path.as_ref());
        match self.hashes.binary_search(&hash) {
            Ok(_) => true,
            Err(idx) => {
                self.hashes.insert(idx, hash);
                false
            }
        }
    }
    pub fn insert(&mut self, path: impl AsRef<Path>) {
        self.contains_or_insert(path);
    }

    pub fn with(self, path: impl AsRef<Path>) -> Self {
        let mut this = self;
        this.insert(path);
        this
    }

    pub fn contains(&self, path: impl AsRef<Path>) -> bool {
        self.hashes
            .binary_search(&self.hash_path(path.as_ref()))
            .is_ok()
    }
}

fn get_xdg_config_files() -> Vec<PathBuf> {
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
        Err(std::env::VarError::NotPresent) => std::iter::once("/etc/xdg/".to_string()).right(),
        Err(std::env::VarError::NotUnicode(_raw)) => {
            todo!();
        }
        Ok(s) => s.split_owned(":").left(),
    };
    let xdg_config_dirs = xdg_config_dirs_raw.map(|s| PathBuf::from(s));
    xdg_config_dirs
}
