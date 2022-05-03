use crate::{result::{to_rv, RvError, RvResult}, cache::FileCacheCfgArgs};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::{
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
};

const CFG_TOML_PATH: &str = "rv_cfg.toml";
const CFG_DEFAULT: &str = r#"
    connection = "Local" # "Local" or "Ssh"
    cache = "FileCache"  # "NoCache" or "FileCache"
    [file_cache_args]
    n_prev_images = 2
    n_next_images = 8
    n_threads = 2 
    # tmpdir = 
    [ssh_cfg]
    remote_folder_path = "a/b/c"
    address = "73.42.73.42"
    user = "someuser"
    ssh_identity_file_path = "local/path"
    "#;

lazy_static! {
    static ref DEFAULT_TMPDIR: PathBuf = std::env::temp_dir().join("rvimage");
}

pub fn get_default_cfg() -> Cfg {
    toml::from_str(CFG_DEFAULT).expect("default config broken")
}

pub fn get_cfg() -> RvResult<Cfg> {
    if Path::new(CFG_TOML_PATH).exists() {
        let toml_str = fs::read_to_string(CFG_TOML_PATH).map_err(to_rv)?;
        toml::from_str(&toml_str).map_err(to_rv)
    } else {
        Ok(get_default_cfg())
    }
}


#[derive(Deserialize, Debug)]
pub enum Connection {
    Ssh,
    Local,
}
#[derive(Deserialize, Debug)]
pub enum Cache {
    FileCache,
    NoCache,
}
#[derive(Deserialize, Debug, Clone)]
pub struct SshCfg {
    pub remote_folder_path: String,
    pub address: String,
    pub user: String,
    pub ssh_identity_file_path: String,
    n_reconnection_attempts: Option<usize>
}
impl SshCfg {
    pub fn n_reconnection_attempts(&self) -> usize {
        match self.n_reconnection_attempts {
            Some(n) => n,
            None => 5
        }
    }
}
#[derive(Deserialize, Debug)]
pub struct Cfg {
    pub connection: Connection,
    pub cache: Cache,
    pub file_cache_args: Option<FileCacheCfgArgs>,
    tmpdir: Option<String>,
    pub ssh_cfg: SshCfg,
}
impl Cfg {
    pub fn tmpdir(&self) -> RvResult<&str> {
        match &self.tmpdir {
            Some(td) => Ok(td.as_str()),
            None => DEFAULT_TMPDIR
                .to_str()
                .ok_or_else(||RvError::new("could not get tmpdir")),
        }
    }
}

#[test]
fn test_toml() -> RvResult<()> {
    let cfg: Cfg = get_cfg()?;
    println!("{:?}", cfg);
    Ok(())
}
