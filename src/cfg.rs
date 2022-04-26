use crate::{result::{to_rv, RvError, RvResult}, cache::FileCacheArgs};
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
    # Path to your scp command including the terminal application, e.g., bash or cmd.
    # If not given, we use 
    # cmd /C scp
    # on Windows and 
    # sh -c scp
    # otherwise.
    # scp_command = 
    "#;

lazy_static! {
    static ref DEFAULT_TMPDIR: PathBuf = std::env::temp_dir().join("rvimage");
}

const SSH_CMD_ARR: [&str; 3] = if cfg!(target_os = "windows") {
    ["cmd", "/C", "ssh"]
} else {
    ["sh", "-c", "ssh"]
};

const SCP_CMD_ARR: [&str; 3] = if cfg!(target_os = "windows") {
    ["cmd", "/C", "scp"]
} else {
    ["sh", "-c", "scp"]
};

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

fn to_vec_string(strs: &[&str]) -> Vec<String> {
    strs.iter().map(|s| s.to_string()).collect::<Vec<_>>()
}

fn ssh_default_cmd() -> &'static [String] {
    lazy_static! {
        pub static ref DEFAULT: Vec<String> = to_vec_string(&SSH_CMD_ARR);
    }
    &DEFAULT
}

fn scp_default_cmd() -> &'static [String] {
    lazy_static! {
        pub static ref DEFAULT: Vec<String> = to_vec_string(&SCP_CMD_ARR);
    }
    &DEFAULT
}

fn unpack_cmd<'a>(cmd: &'a Option<Vec<String>>, default_cmd: &'static [String]) -> &'a [String] {
    match cmd {
        Some(s) => s,
        None => default_cmd,
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
#[derive(Deserialize, Debug)]
pub struct SshCfg {
    pub remote_folder_path: String,
    pub address: String,
    pub user: String,
    pub ssh_identity_file_path: String,
    scp_cmd: Option<Vec<String>>,
    ssh_cmd: Option<Vec<String>>,
}
impl SshCfg {
    pub fn ssh_cmd(&self) -> &[String] {
        unpack_cmd(&self.ssh_cmd, ssh_default_cmd())
    }
    pub fn scp_cmd(&self) -> &[String] {
        unpack_cmd(&self.scp_cmd, scp_default_cmd())
    }
}
#[derive(Deserialize, Debug)]
pub struct Cfg {
    pub connection: Connection,
    pub cache: Cache,
    pub file_cache_args: Option<FileCacheArgs>,
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
#[test]
fn test_cmd() -> RvResult<()> {
    let cfg = get_default_cfg();
    assert_eq!(None, cfg.ssh_cfg.ssh_cmd);
    assert_eq!(None, cfg.ssh_cfg.scp_cmd);
    if cfg!(target_os = "windows") {
        assert_eq!(["cmd", "/C", "ssh"], cfg.ssh_cfg.ssh_cmd());
        assert_eq!(["cmd", "/C", "scp"], cfg.ssh_cfg.scp_cmd());
    } else {
        assert_eq!(["sh", "cc", "ssh"], cfg.ssh_cfg.ssh_cmd());
        assert_eq!(["sh", "-c", "scp"], cfg.ssh_cfg.scp_cmd());
    }
    Ok(())
}
