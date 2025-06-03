use crate::{
    cache::FileCacheCfgArgs,
    file_util::{self, path_to_str, DEFAULT_PRJ_PATH, DEFAULT_TMPDIR},
    result::trace_ok_err,
    sort_params::SortParams,
    ssh,
};
use rvimage_domain::{rverr, to_rv, RvResult};
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
            file_cache_args: self.file_cache_args.unwrap_or_default(),
            image_change_delay_on_held_key_ms: get_image_change_delay_on_held_key_ms(),

            ssh: SshCfgUsr {
                user: self.ssh_cfg.user,
                ssh_identity_file_path: self.ssh_cfg.ssh_identity_file_path,
                n_reconnection_attempts: self.ssh_cfg.n_reconnection_attempts,
            },
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
                connection_string_path: ab.connection_string_path,
                container_name: ab.container_name,
                prefix: ab.prefix,
            }),
            sort_params: SortParams::default(),
        };
        Cfg { usr, prj }
    }
}

pub fn get_cfg_path_legacy(homefolder: &Path) -> PathBuf {
    homefolder.join("rv_cfg.toml")
}

pub fn get_cfg_path_usr(homefolder: &Path) -> PathBuf {
    homefolder.join("rv_cfg_usr.toml")
}

pub fn get_cfg_path_prj(homefolder: &Path) -> PathBuf {
    homefolder.join("rv_cfg_prjtmp.toml")
}

pub fn get_cfg_tmppath(cfg: &Cfg) -> PathBuf {
    Path::new(cfg.tmpdir())
        .join(".rvimage")
        .join("rv_cfg_tmp.toml")
}

pub fn get_log_folder(homefolder: &Path) -> PathBuf {
    homefolder.join("logs")
}

fn parse_toml_str<CFG: Debug + DeserializeOwned + Default>(toml_str: &str) -> RvResult<CFG> {
    match toml::from_str(&toml_str) {
        Ok(cfg) => Ok(cfg),
        Err(_) => {
            // lets try replacing \ by / and see if we can parse it
            let toml_str = toml_str.replace('\\', "/");
            match toml::from_str(&toml_str) {
                Ok(cfg) => Ok(cfg),
                Err(_) => {
                    // lets try replacing " by ' and see if we can parse it
                    let toml_str = toml_str.replace('"', "'");
                    toml::from_str(&toml_str)
                        .map_err(|e| rverr!("failed to parse cfg due to {e:?}"))
                }
            }
        }
    }
}

pub fn read_cfg_gen<CFG: Debug + DeserializeOwned + Default>(
    cfg_toml_path: &Path,
) -> RvResult<CFG> {
    if cfg_toml_path.exists() {
        let toml_str = file_util::read_to_string(cfg_toml_path)?;
        parse_toml_str(&toml_str)
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
        Ok(Cfg::default())
    }
}

pub fn write_cfg_str(cfg_str: &str, p: &Path, log: bool) -> RvResult<()> {
    file_util::write(p, cfg_str)?;
    if log {
        info!("wrote cfg to {p:?}");
    }
    Ok(())
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
pub struct AzureBlobCfgPrj {
    #[serde(default)]
    pub connection_string_path: String,
    pub container_name: String,
    pub prefix: String,
}

#[cfg(feature = "azure_blob")]
#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct AzureBlobCfg {
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
    pub fn read(&self, src_path: &Path, ssh_cfg: Option<&SshCfg>) -> RvResult<String> {
        match (self, ssh_cfg) {
            (ExportPathConnection::Ssh, Some(ssh_cfg)) => {
                let sess = ssh::auth(ssh_cfg)?;
                let read_bytes = ssh::download(path_to_str(src_path)?, &sess)?;
                String::from_utf8(read_bytes).map_err(to_rv)
            }
            (ExportPathConnection::Local, _) => file_util::read_to_string(src_path),
            (ExportPathConnection::Ssh, None) => {
                Err(rverr!("cannot read from ssh. config missing"))
            }
        }
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

fn get_image_change_delay_on_held_key_ms() -> u64 {
    300
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct CfgUsr {
    pub darkmode: Option<bool>,
    #[serde(default = "get_default_n_autosaves")]
    pub n_autosaves: Option<u8>,

    #[serde(default = "get_image_change_delay_on_held_key_ms")]
    pub image_change_delay_on_held_key_ms: u64,

    // This is only variable to make the CLI and tests not override your config.
    // You shall not change this when actually running RV Image.
    pub home_folder: Option<String>,

    pub cache: Cache,
    tmpdir: Option<String>,
    current_prj_path: Option<PathBuf>,
    #[serde(default)]
    pub file_cache_args: FileCacheCfgArgs,
    pub ssh: SshCfgUsr,
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct CfgPrj {
    pub py_http_reader_cfg: Option<PyHttpReaderCfg>,
    pub connection: Connection,
    http_address: Option<String>,
    pub ssh: SshCfgPrj,
    #[cfg(feature = "azure_blob")]
    pub azure_blob: Option<AzureBlobCfgPrj>,
    #[serde(default)]
    pub sort_params: SortParams,
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Cfg {
    pub usr: CfgUsr,
    pub prj: CfgPrj,
}

impl Cfg {
    /// for multiple cli instances to run in parallel
    pub fn with_unique_folders() -> Self {
        let mut cfg = Self::default();
        let uuid_str = format!("{}", uuid::Uuid::new_v4());
        let tmpdir_str = DEFAULT_TMPDIR
            .to_str()
            .expect("default tmpdir does not exist. cannot work without")
            .to_string();
        cfg.usr.tmpdir = Some(format!("{tmpdir_str}/rvimage_tmp_{uuid_str}"));
        let tmp_homedir = format!("{tmpdir_str}/rvimage_home_{uuid_str}");

        // copy user cfg to tmp homedir
        trace_ok_err(fs::create_dir_all(&tmp_homedir));
        if let Some(home_folder) = &cfg.usr.home_folder {
            let usrcfg_path = get_cfg_path_usr(Path::new(home_folder));
            if usrcfg_path.exists() {
                if let Some(filename) = usrcfg_path.file_name() {
                    trace_ok_err(fs::copy(
                        &usrcfg_path,
                        Path::new(&tmp_homedir).join(filename),
                    ));
                }
            }
        }
        cfg.usr.home_folder = Some(tmp_homedir);
        cfg
    }
    pub fn ssh_cfg(&self) -> SshCfg {
        SshCfg {
            usr: self.usr.ssh.clone(),
            prj: self.prj.ssh.clone(),
        }
    }
    #[cfg(feature = "azure_blob")]
    pub fn azure_blob_cfg(&self) -> Option<AzureBlobCfg> {
        self.prj
            .azure_blob
            .as_ref()
            .map(|prj| AzureBlobCfg { prj: prj.clone() })
    }
    pub fn home_folder(&self) -> &str {
        let ef = self.usr.home_folder.as_deref();
        match ef {
            None => file_util::get_default_homedir(),
            Some(ef) => ef,
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
    pub fn unset_current_prj_path(&mut self) {
        self.usr.current_prj_path = None;
    }

    pub fn write(&self) -> RvResult<()> {
        let homefolder = Path::new(self.home_folder());
        let cfg_usr_path = get_cfg_path_usr(homefolder);
        if let Some(cfg_parent) = cfg_usr_path.parent() {
            fs::create_dir_all(cfg_parent).map_err(to_rv)?;
        }
        let cfg_usr_str = toml::to_string_pretty(&self.usr).map_err(to_rv)?;
        let log = true;
        write_cfg_str(&cfg_usr_str, &cfg_usr_path, log).and_then(|_| {
            let cfg_prj_path = get_cfg_path_prj(homefolder);
            let cfg_prj_str = toml::to_string_pretty(&self.prj).map_err(to_rv)?;
            write_cfg_str(&cfg_prj_str, &cfg_prj_path, log)
        })
    }
    pub fn read(homefolder: &Path) -> RvResult<Self> {
        let cfg_toml_path_usr = get_cfg_path_usr(homefolder);
        let cfg_toml_path_prj = get_cfg_path_prj(homefolder);
        let cfg_toml_path_legacy = get_cfg_path_legacy(homefolder);
        read_cfg_from_paths(
            &cfg_toml_path_usr,
            &cfg_toml_path_prj,
            &cfg_toml_path_legacy,
        )
    }
}
impl Default for Cfg {
    fn default() -> Self {
        let usr = CfgUsr::default();
        let prj = CfgPrj::default();
        let mut cfg = Cfg { usr, prj };
        cfg.usr.current_prj_path = Some(DEFAULT_PRJ_PATH.to_path_buf());
        cfg
    }
}
#[cfg(test)]
use file_util::get_default_homedir;

#[test]
fn test_default_cfg_paths() {
    get_default_homedir();
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

#[cfg(test)]
fn make_cfg_str(ssh_identity_filepath: &str) -> String {
    let part1 = r#"
[usr]
n_autosaves = 10
image_change_delay_on_held_key_ms = 10
cache = "FileCache"
current_prj_path = "someprjpath.json"

[usr.file_cache_args]
n_prev_images = 4
n_next_images = 8
n_threads = 2
clear_on_close = true
cachedir = "C:/Users/ShafeiB/.rvimage/cache"

[usr.ssh]
user = "auser"
ssh_identity_file_path ="#;

    let part2 = r#"
[prj]
connection = "Local"

[prj.py_http_reader_cfg]
server_addresses = [
    "http://localhost:8000/somewhere",
    "http://localhost:8000/elsewhere",
]

[prj.ssh]
remote_folder_paths = ["/"]
address = "12.11.10.13:22"

[prj.sort_params]
kind = "Natural"
sort_by_filename = false
"#;

    format!("{part1} {ssh_identity_filepath} {part2}")
}

#[test]
fn test_parse_toml() {
    fn test(ssh_path: &str, ssh_path_expected: &str) {
        let toml_str = make_cfg_str(ssh_path);
        let cfg: Cfg = parse_toml_str(&toml_str).unwrap();
        assert_eq!(
            cfg.usr.ssh.ssh_identity_file_path,
            ssh_path_expected);
    }
    test("\"c:\\somehome\\.ssh\\id_rsa\"", "c:/somehome/.ssh/id_rsa");
    test("'c:\\some home\\.ssh\\id_rsa'", "c:\\some home\\.ssh\\id_rsa");
    test("\"/s omehome\\.ssh\\id_rsa\"", "/s omehome/.ssh/id_rsa");
    test("'/some home/.ssh/id_rsa'", "/some home/.ssh/id_rsa");
}
