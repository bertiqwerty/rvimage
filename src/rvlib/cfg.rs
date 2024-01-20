use crate::{
    cache::FileCacheCfgArgs,
    file_util::{self, DEFAULT_HOMEDIR, DEFAULT_PRJ_PATH, DEFAULT_TMPDIR},
    result::{to_rv, RvError, RvResult},
    rverr, ssh,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
};
use tracing::{error, info};

const CFG_DEFAULT: &str = r#"
    connection = "Local" # "Local" or "Ssh"
    cache = "FileCache"  # "NoCache" or "FileCache" 
    current_prj_path = "default.rvi"
    n_autosaves = 2
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
    let mut cfg: Cfg = toml::from_str(CFG_DEFAULT).expect("default config broken");
    cfg.current_prj_path = Some(DEFAULT_PRJ_PATH.to_path_buf());
    cfg
}

pub fn get_cfg_path() -> RvResult<PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| RvError::new("where is your home? cannot load config"))?
        .join(".rvimage")
        .join("rv_cfg.toml"))
}

pub fn get_log_folder() -> RvResult<PathBuf> {
    get_cfg_path().and_then(|cfg_path| {
        Ok(cfg_path
            .parent()
            .ok_or_else(|| RvError::new("the cfg file needs a parent"))?
            .join("logs"))
    })
}

pub fn read_cfg() -> RvResult<Cfg> {
    let cfg_toml_path = get_cfg_path()?;
    if cfg_toml_path.exists() {
        let toml_str = file_util::read_to_string(cfg_toml_path)?;
        match toml::from_str(&toml_str).map_err(|e| rverr!("could not parse cfg due to {:?}", e)) {
            Ok(cfg) => Ok(cfg),
            Err(e) => {
                error!("could not parse cfg due to {e:?}.");
                error!("using default cfg");
                Ok(get_default_cfg())
            }
        }
    } else {
        Ok(get_default_cfg())
    }
}

pub fn write_cfg(cfg: &Cfg) -> RvResult<()> {
    let cfg_path = get_cfg_path()?;
    if let Some(cfg_parent) = cfg_path.parent() {
        fs::create_dir_all(cfg_parent).map_err(to_rv)?;
    }
    let cfg_str = toml::to_string_pretty(cfg).map_err(to_rv)?;
    write_cfg_str(&cfg_str)
}

pub fn write_cfg_str(cfg_str: &str) -> RvResult<()> {
    let cfg_toml_path = get_cfg_path()?;
    file_util::write(cfg_toml_path.clone(), cfg_str)?;
    info!("wrote cfg to {cfg_toml_path:?}");
    Ok(())
}

pub fn read_darkmode() -> Option<bool> {
    read_cfg().ok().and_then(|cfg| cfg.darkmode)
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq, Clone, Copy, Default)]
pub enum Connection {
    Ssh,
    PyHttp,
    #[cfg(feature = "azure_blob")]
    AzureBlob,
    #[default]
    Local,
}
#[derive(Deserialize, Serialize, Debug, Eq, PartialEq, Clone, Default)]
pub enum Cache {
    #[default]
    FileCache,
    NoCache,
}
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
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

#[cfg(feature = "azure_blob")]
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct AzureBlobCfg {
    pub connection_string_path: String,
    pub container_name: String,
    pub prefix: String,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct PyHttpReaderCfg {
    pub server_address: String,
}

#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub enum ExportPathConnection {
    Ssh,
    #[default]
    Local,
}
impl ExportPathConnection {
    pub fn write_bytes(
        &self,
        data: &[u8],
        dst_path: &Path,
        ssh_cfg: Option<&SshCfg>,
    ) -> RvResult<()> {
        match (self, ssh_cfg) {
            (ExportPathConnection::Ssh, Some(ssh_cfg)) => {
                let sess = ssh::auth(ssh_cfg)?;
                ssh::write_bytes(data, dst_path, &sess).map_err(to_rv)?;
                Ok(())
            }
            (ExportPathConnection::Local, _) => {
                file_util::write(dst_path, data)?;
                Ok(())
            }
            (ExportPathConnection::Ssh, None) => Err(rverr!("cannot save to ssh. config missing")),
        }
    }
    pub fn write(&self, data_str: &str, dst_path: &Path, ssh_cfg: Option<&SshCfg>) -> RvResult<()> {
        self.write_bytes(data_str.as_bytes(), dst_path, ssh_cfg)
    }
}
#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct ExportPath {
    pub path: PathBuf,
    pub conn: ExportPathConnection,
}

pub enum Style {
    Dark,
    Light,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Cfg {
    pub connection: Connection,
    pub cache: Cache,
    http_address: Option<String>,
    tmpdir: Option<String>,
    current_prj_path: Option<PathBuf>,
    pub file_cache_args: Option<FileCacheCfgArgs>,
    pub ssh_cfg: SshCfg,
    pub home_folder: Option<String>,
    pub py_http_reader_cfg: Option<PyHttpReaderCfg>,
    pub darkmode: Option<bool>,
    pub n_autosaves: Option<u8>,
    #[cfg(feature = "azure_blob")]
    pub azure_blob_cfg: Option<AzureBlobCfg>,
}

impl Cfg {
    pub fn home_folder(&self) -> RvResult<&str> {
        let ef = self.home_folder.as_deref();
        match ef {
            None => DEFAULT_HOMEDIR
                .to_str()
                .ok_or_else(|| RvError::new("could not get homedir")),
            Some(ef) => Ok(ef),
        }
    }

    pub fn tmpdir(&self) -> RvResult<&str> {
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

    pub fn current_prj_path(&self) -> &Path {
        if let Some(pp) = &self.current_prj_path {
            pp
        } else {
            &DEFAULT_PRJ_PATH
        }
    }
    pub fn set_current_prj_path(&mut self, pp: PathBuf) {
        self.current_prj_path = Some(pp);
    }
}

#[test]
fn test_toml() -> RvResult<()> {
    let cfg: Cfg = read_cfg()?;
    println!("{:?}", cfg);
    Ok(())
}
