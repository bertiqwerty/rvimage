use crate::{
    cache::FileCacheCfgArgs,
    result::{to_rv, RvError, RvResult},
};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, fs, path::PathBuf};

const CFG_DEFAULT: &str = r#"
    connection = "Local" # "Local" or "Ssh"
    cache = "FileCache"  # "NoCache" or "FileCache" 
    [file_cache_args]
    n_prev_images = 2
    n_next_images = 8
    n_threads = 2 
    # tmpdir = 
    [ssh_cfg]
    remote_folder_paths = ["a/b/c"]
    address = "73.42.73.42"
    user = "someuser"
    ssh_identity_file_path = "local/path"
    "#;

pub fn get_default_cfg() -> Cfg {
    toml::from_str(CFG_DEFAULT).expect("default config broken")
}

pub fn get_cfg_path() -> RvResult<PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| RvError::new("where is your home? cannot load config"))?
        .join(".rvimage")
        .join("rv_cfg.toml"))
}

pub fn get_cfg() -> RvResult<Cfg> {
    let cfg_toml_path = get_cfg_path()?;
    if cfg_toml_path.exists() {
        let toml_str = fs::read_to_string(cfg_toml_path).map_err(to_rv)?;
        toml::from_str(&toml_str).map_err(to_rv)
    } else {
        Ok(get_default_cfg())
    }
}

pub fn write_cfg(cfg: &Cfg) -> RvResult<()> {
    let cfg_str = toml::to_string_pretty(cfg).map_err(to_rv)?;
    write_cfg_str(&cfg_str)
}

pub fn write_cfg_str(cfg_str: &str) -> RvResult<()> {
    let cfg_toml_path = get_cfg_path()?;
    fs::write(cfg_toml_path, cfg_str).map_err(to_rv)?;
    Ok(())
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq, Clone, Copy)]
pub enum Connection {
    Ssh,
    Local,
}
#[derive(Deserialize, Serialize, Debug, Eq, PartialEq, Clone)]
pub enum Cache {
    FileCache,
    NoCache,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SshCfg {
    pub remote_folder_paths: Vec<String>,
    pub address: String,
    pub user: String,
    pub ssh_identity_file_path: String,
    n_reconnection_attempts: Option<usize>,
}
impl SshCfg {
    pub fn n_reconnection_attempts(&self) -> usize {
        let default = 5;
        self.n_reconnection_attempts.unwrap_or(default)
    }
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Cfg {
    pub connection: Connection,
    pub cache: Cache,
    http_address: Option<String>,
    tmpdir: Option<String>,
    pub file_cache_args: Option<FileCacheCfgArgs>,
    pub ssh_cfg: SshCfg,
}
impl Cfg {
    pub fn tmpdir(&self) -> RvResult<&str> {
        lazy_static! {
            static ref DEFAULT_TMPDIR: PathBuf = std::env::temp_dir().join("rvimage");
        }
        match &self.tmpdir {
            Some(td) => Ok(td.as_str()),
            None => DEFAULT_TMPDIR
                .to_str()
                .ok_or_else(|| RvError::new("could not get tmpdir")),
        }
    }
    pub fn http_address(&self) -> &str {
        match &self.http_address {
            Some(http_addr) => http_addr,
            None => "127.0.0.1:5432",
        }
    }
}

#[test]
fn test_toml() -> RvResult<()> {
    let cfg: Cfg = get_cfg()?;
    println!("{:?}", cfg);
    Ok(())
}
