use serde::Deserialize;
use std::{fmt::Debug, fs, path::Path};
use crate::result::{RvResult, to_rv};

const CFG_TOML_PATH: &str = "cfg.toml";
const CFG_DEFAULT: &str = r#"
    # "LocalReader" or "ScpReader"
    connection = "LocalReader"
    [scp_cfg]
    remote_folder_path = "a/b/c"
    address = "10.100.9.8"
    user = "someuser"
    ssh_key_file_path = "local/path"
    "#;

pub fn get_cfg() -> RvResult<Cfg> {
    if Path::new(CFG_TOML_PATH).exists() {
        let toml_str = fs::read_to_string(CFG_TOML_PATH).map_err(to_rv)?;
        toml::from_str(&toml_str).map_err(to_rv)
    } else {
        toml::from_str(CFG_DEFAULT).map_err(to_rv)
    }
}

#[derive(Deserialize, Debug)]
pub enum Connection {
    ScpReader,
    LocalReader,
}

#[derive(Deserialize, Debug)]
pub struct ScpCfg {
    pub remote_folder_path: String,
    pub address: String,
    pub user: String,
    pub ssh_key_file_path: String,
}

#[derive(Deserialize, Debug)]
pub struct Cfg {
    pub connection: Connection,
    pub scp_cfg: ScpCfg,
}

#[test]
fn test_toml() -> RvResult<()> {
    let cfg: Cfg = get_cfg()?;
    println!("{:?}", cfg);
    Ok(())
}
