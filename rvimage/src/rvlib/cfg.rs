use crate::{
    cache::FileCacheCfgArgs,
    file_util::{self, DEFAULT_HOMEDIR, DEFAULT_PRJ_PATH, DEFAULT_TMPDIR},
    ssh,
};
use rvimage_domain::{rverr, to_rv, RvError, RvResult};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
};
use tracing::{info, warn};

#[cfg(feature = "azure_blob")]
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct AzureBlobCfgLegacy {
    pub connection_string_path: String,
    pub container_name: String,
    pub prefix: String,
}
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct SshCfgLegacy {
    pub user: String,
    pub ssh_identity_file_path: String,
    n_reconnection_attempts: Option<usize>,
    pub remote_folder_paths: Vec<String>,
    pub address: String,
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct CfgLegacy {
    pub connection: Connection,
    pub cache: Cache,
    http_address: Option<String>,
    tmpdir: Option<String>,
    current_prj_path: Option<PathBuf>,
    pub file_cache_args: Option<FileCacheCfgArgs>,
    pub ssh_cfg: SshCfgLegacy,
    pub home_folder: Option<String>,
    pub py_http_reader_cfg: Option<PyHttpReaderCfg>,
    pub darkmode: Option<bool>,
    pub n_autosaves: Option<u8>,
    pub import_old_path: Option<String>,
    pub import_new_path: Option<String>,
    #[cfg(feature = "azure_blob")]
    pub azure_blob_cfg: Option<AzureBlobCfgLegacy>,
}
impl CfgLegacy {
    pub fn to_cfg(self) -> Cfg {
        let usr = CfgUsr {
            darkmode: self.darkmode,
            n_autosaves: self.n_autosaves,
            home_folder: self.home_folder,
            cache: self.cache,
            tmpdir: self.tmpdir,
            current_prj_path: self.current_prj_path,
            file_cache_args: self.file_cache_args,
            ssh: SshCfgUsr {
                user: self.ssh_cfg.user,
                ssh_identity_file_path: self.ssh_cfg.ssh_identity_file_path,
                n_reconnection_attempts: self.ssh_cfg.n_reconnection_attempts,
            },
            azure_blob: self.azure_blob_cfg.clone().map(|ab| AzureBlobCfgUsr {
                connection_string_path: ab.connection_string_path,
            }),
        };
        let prj = CfgPrj {
            connection: self.connection,
            http_address: self.http_address,
            py_http_reader_cfg: self.py_http_reader_cfg,
            ssh: SshCfgPrj {
                remote_folder_paths: self.ssh_cfg.remote_folder_paths,
                address: self.ssh_cfg.address,
            },
            azure_blob: self.azure_blob_cfg.map(|ab| AzureBlobCfgPrj {
                container_name: ab.container_name,
                prefix: ab.prefix,
            }),
        };
        Cfg { usr, prj }
    }
}

const CFG_DEFAULT_USR: &str = r#"
    cache = "FileCache"  # "NoCache" or "FileCache" 
    current_prj_path = "default.rvi"
    n_autosaves = 2
    [file_cache_args]
    n_prev_images = 2
    n_next_images = 8
    n_threads = 2 
    # tmpdir = 
    [ssh]
    user = "someuser"
    ssh_identity_file_path = "local/path"
    "#;

const CFG_DEFAULT_PRJ: &str = r#"
    connection = "Local" # "Local" or "Ssh"
    [ssh]
    remote_folder_paths = ["a/b/c"]
    address = "73.42.73.42"
    "#;

fn get_default_cfg_usr() -> CfgUsr {
    toml::from_str(CFG_DEFAULT_USR).expect("default user config broken")
}

fn get_default_cfg_prj() -> CfgPrj {
    toml::from_str(CFG_DEFAULT_PRJ).expect("default prj config broken")
}

pub fn get_default_cfg() -> Cfg {
    let usr = get_default_cfg_usr();
    let prj = get_default_cfg_prj();
    let mut cfg = Cfg { usr, prj };
    cfg.usr.current_prj_path = Some(DEFAULT_PRJ_PATH.to_path_buf());
    cfg
}
fn get_cfg_path(filename: &str) -> RvResult<PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| RvError::new("where is your home? cannot load config"))?
        .join(".rvimage")
        .join(filename))
}
pub fn get_cfg_path_legacy() -> RvResult<PathBuf> {
    get_cfg_path("rv_cfg.toml")
}

pub fn get_cfg_path_usr() -> RvResult<PathBuf> {
    get_cfg_path("rv_cfg_usr.toml")
}

pub fn get_cfg_path_prj() -> RvResult<PathBuf> {
    get_cfg_path("rv_cfg_prjtmp.toml")
}

pub fn get_cfg_tmppath(cfg: &Cfg) -> PathBuf {
    Path::new(cfg.tmpdir())
        .join(".rvimage")
        .join("rv_cfg_tmp.toml")
}

pub fn get_log_folder() -> RvResult<PathBuf> {
    get_cfg_path_usr().and_then(|cfg_path| {
        Ok(cfg_path
            .parent()
            .ok_or_else(|| RvError::new("the cfg file needs a parent"))?
            .join("logs"))
    })
}

pub fn read_cfg_gen<CFG: Debug + DeserializeOwned + Default>(
    cfg_toml_path: &Path,
) -> RvResult<CFG> {
    if cfg_toml_path.exists() {
        let toml_str = file_util::read_to_string(cfg_toml_path)?;
        toml::from_str(&toml_str).map_err(|e| rverr!("could not parse cfg due to {:?}", e))
    } else {
        warn!("cfg {cfg_toml_path:?} file does not exist. using default cfg");
        Ok(CFG::default())
    }
}

fn read_cfg_from_paths(
    cfg_toml_path_usr: &Path,
    cfg_toml_path_prj: &Path,
    cfg_toml_path_legacy: &Path,
) -> RvResult<Cfg> {
    if cfg_toml_path_usr.exists() || cfg_toml_path_prj.exists() {
        let usr = read_cfg_gen::<CfgUsr>(cfg_toml_path_usr)?;
        let prj = read_cfg_gen::<CfgPrj>(cfg_toml_path_prj)?;
        Ok(Cfg { usr, prj })
    } else if cfg_toml_path_legacy.exists() {
        tracing::warn!("using legacy cfg file {cfg_toml_path_legacy:?}");
        let legacy = read_cfg_gen::<CfgLegacy>(cfg_toml_path_legacy)?;
        Ok(legacy.to_cfg())
    } else {
        tracing::info!("no cfg file found. using default cfg");
        Ok(get_default_cfg())
    }
}

pub fn read_cfg() -> RvResult<Cfg> {
    let cfg_toml_path_usr = get_cfg_path_usr()?;
    let cfg_toml_path_prj = get_cfg_path_prj()?;
    let cfg_toml_path_legacy = get_cfg_path_legacy()?;
    read_cfg_from_paths(
        &cfg_toml_path_usr,
        &cfg_toml_path_prj,
        &cfg_toml_path_legacy,
    )
}

pub fn write_cfg_str(cfg_str: &str, p: &Path, log: bool) -> RvResult<()> {
    file_util::write(p, cfg_str)?;
    if log {
        info!("wrote cfg to {p:?}");
    }
    Ok(())
}

pub fn write_cfg(cfg: &Cfg) -> RvResult<()> {
    let cfg_usr_path = get_cfg_path_usr()?;
    if let Some(cfg_parent) = cfg_usr_path.parent() {
        fs::create_dir_all(cfg_parent).map_err(to_rv)?;
    }
    let cfg_usr_str = toml::to_string_pretty(&cfg.usr).map_err(to_rv)?;
    let log = true;
    write_cfg_str(&cfg_usr_str, &cfg_usr_path, log).and_then(|_| {
        let cfg_prj_path = get_cfg_path_prj()?;
        let cfg_prj_str = toml::to_string_pretty(&cfg.prj).map_err(to_rv)?;
        write_cfg_str(&cfg_prj_str, &cfg_prj_path, log)
    })
}

pub fn read_darkmode() -> Option<bool> {
    read_cfg().ok().and_then(|cfg| cfg.usr.darkmode)
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
pub struct SshCfgUsr {
    pub user: String,
    pub ssh_identity_file_path: String,
    n_reconnection_attempts: Option<usize>,
}
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct SshCfgPrj {
    pub remote_folder_paths: Vec<String>,
    pub address: String,
}
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct SshCfg {
    pub usr: SshCfgUsr,
    pub prj: SshCfgPrj,
}
impl SshCfg {
    pub fn n_reconnection_attempts(&self) -> usize {
        let default = 5;
        self.usr.n_reconnection_attempts.unwrap_or(default)
    }
}

#[cfg(feature = "azure_blob")]
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct AzureBlobCfgUsr {
    pub connection_string_path: String,
}
#[cfg(feature = "azure_blob")]
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct AzureBlobCfgPrj {
    pub container_name: String,
    pub prefix: String,
}

#[cfg(feature = "azure_blob")]
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct AzureBlobCfg {
    pub user: AzureBlobCfgUsr,
    pub prj: AzureBlobCfgPrj,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct PyHttpReaderCfg {
    pub server_addresses: Vec<String>,
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

fn get_default_n_autosaves() -> Option<u8> {
    Some(2)
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct CfgUsr {
    pub darkmode: Option<bool>,
    #[serde(default = "get_default_n_autosaves")]
    pub n_autosaves: Option<u8>,
    pub home_folder: Option<String>,
    pub cache: Cache,
    tmpdir: Option<String>,
    current_prj_path: Option<PathBuf>,
    pub file_cache_args: Option<FileCacheCfgArgs>,
    pub ssh: SshCfgUsr,
    #[cfg(feature = "azure_blob")]
    pub azure_blob: Option<AzureBlobCfgUsr>,
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct CfgPrj {
    pub py_http_reader_cfg: Option<PyHttpReaderCfg>,
    pub connection: Connection,
    http_address: Option<String>,
    pub ssh: SshCfgPrj,
    #[cfg(feature = "azure_blob")]
    pub azure_blob: Option<AzureBlobCfgPrj>,
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Cfg {
    pub usr: CfgUsr,
    pub prj: CfgPrj,
}

impl Cfg {
    pub fn ssh_cfg(&self) -> SshCfg {
        SshCfg {
            usr: self.usr.ssh.clone(),
            prj: self.prj.ssh.clone(),
        }
    }
    #[cfg(feature = "azure_blob")]
    pub fn azure_blob_cfg(&self) -> Option<AzureBlobCfg> {
        match (&self.usr.azure_blob, &self.prj.azure_blob) {
            (Some(user), Some(prj)) => Some(AzureBlobCfg {
                user: user.clone(),
                prj: prj.clone(),
            }),
            _ => None,
        }
    }
    pub fn home_folder(&self) -> RvResult<&str> {
        let ef = self.usr.home_folder.as_deref();
        match ef {
            None => DEFAULT_HOMEDIR
                .to_str()
                .ok_or_else(|| RvError::new("could not get homedir")),
            Some(ef) => Ok(ef),
        }
    }

    pub fn tmpdir(&self) -> &str {
        match &self.usr.tmpdir {
            Some(td) => td.as_str(),
            None => DEFAULT_TMPDIR.to_str().unwrap(),
        }
    }

    pub fn http_address(&self) -> &str {
        match &self.prj.http_address {
            Some(http_addr) => http_addr,
            None => "127.0.0.1:5432",
        }
    }

    pub fn current_prj_path(&self) -> &Path {
        if let Some(pp) = &self.usr.current_prj_path {
            pp
        } else {
            &DEFAULT_PRJ_PATH
        }
    }
    pub fn set_current_prj_path(&mut self, pp: PathBuf) {
        self.usr.current_prj_path = Some(pp);
    }
}

#[test]
fn test_toml() -> RvResult<()> {
    let cfg: Cfg = read_cfg()?;
    println!("{:?}", cfg);
    get_default_cfg();
    Ok(())
}
#[test]
fn test_default_cfg_paths() {
    DEFAULT_HOMEDIR.to_str().unwrap();
    DEFAULT_PRJ_PATH.to_str().unwrap();
    DEFAULT_TMPDIR.to_str().unwrap();
}

#[test]
fn test_read_cfg_legacy() {
    let test_folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test_data");
    let cfg_toml_path_usr = test_folder.join("rv_cfg_usr_doesntexist.toml");
    let cfg_toml_path_prj = test_folder.join("rv_cfg_prj_doesntexist.toml");
    let cfg_toml_path_legacy = test_folder.join("rv_cfg_legacy.toml");
    let cfg = read_cfg_from_paths(
        &cfg_toml_path_usr,
        &cfg_toml_path_prj,
        &cfg_toml_path_legacy,
    )
    .unwrap();
    assert_eq!(
        cfg.usr.current_prj_path,
        Some(PathBuf::from("/Users/ultrauser/Desktop/ultra.json"))
    );
    assert_eq!(cfg.usr.darkmode, Some(true));
    assert_eq!(cfg.usr.ssh.user, "someuser");
    assert_eq!(cfg.prj.ssh.address, "73.42.73.42")
}
