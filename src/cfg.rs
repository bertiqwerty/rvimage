use crate::result::{to_rv, RvError, RvResult};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::{
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
};

const CFG_TOML_PATH: &str = "cfg.toml";
const CFG_DEFAULT: &str = r#"
    connection = "Local" # "Local" or "Scp"
    cache = "FileCache"  # "NoCache" or "FileCache"
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
    static ref DEFAULT_TMPDIR: PathBuf = std::env::temp_dir().join("rimview");
}

pub fn get_default_cfg() -> Cfg {
    toml::from_str(CFG_DEFAULT).expect("default config broken.")
}

pub fn get_cfg() -> RvResult<Cfg> {
    if Path::new(CFG_TOML_PATH).exists() {
        let toml_str = fs::read_to_string(CFG_TOML_PATH).map_err(to_rv)?;
        toml::from_str(&toml_str).map_err(to_rv)
    } else {
        Ok(get_default_cfg())
    }
}

fn unpack_cmd<'a>(cmd: &Option<&'a str>, default: &'a str) -> &'a str {
    match cmd {
        Some(s) => s,
        None => default,
    }
}

#[derive(Deserialize, Debug)]
pub enum Connection {
    Scp,
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
    scp_cmd: Option<String>,
    ssh_cmd: Option<String>,
    terminal: Option<String>,
}
impl SshCfg {
    pub fn ssh_cmd(&self) -> &str {
        unpack_cmd(&self.ssh_cmd.as_deref(), "ssh")
    }
    pub fn scp_cmd(&self) -> &str {
        unpack_cmd(&self.scp_cmd.as_deref(), "scp")
    }
    pub fn terminal(&self) -> &str {
        match &self.terminal {
            Some(s) => s,
            None => {
                if cfg!(target_os = "windows") {
                    "cmd /C"
                } else {
                    "sh -c"
                }
            }
        }
    }
}
#[derive(Deserialize, Debug)]
pub struct Cfg {
    pub connection: Connection,
    pub cache: Cache,
    tmpdir: Option<String>,
    pub ssh_cfg: SshCfg,
}
impl Cfg {
    pub fn tmpdir(&self) -> RvResult<&str> {
        match &self.tmpdir {
            Some(td) => Ok(td.as_str()),
            None => DEFAULT_TMPDIR
                .to_str()
                .ok_or(RvError::new("could not get tmpdir")),
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
    assert_eq!("ssh", cfg.ssh_cfg.ssh_cmd());
    assert_eq!("scp", cfg.ssh_cfg.scp_cmd());
    Ok(())
}
