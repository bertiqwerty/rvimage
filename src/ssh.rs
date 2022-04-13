use std::{path::Path, process::Command};

use lazy_static::lazy_static;

use crate::{
    cfg::SshCfg,
    result::{to_rv, RvError, RvResult},
};
use regex::Regex;
fn get_sep(path_to_foler: &str) -> &'static str {
    if path_to_foler.ends_with('/') {
        ""
    } else {
        "/"
    }
}

fn make_cmd<'a, I: Iterator<Item = &'a str>>(mut cmd_iter: I) -> RvResult<Command> {
    let terminal = cmd_iter
        .next()
        .ok_or_else(||RvError::new("couldn't unpack terminal"))?;
    let mut cmd = Command::new(terminal);
    cmd.args(cmd_iter);
    Ok(cmd)
}

fn make_ssh_auth_args(ssh_cfg: &SshCfg) -> [String; 2] {
    ["-i".to_string(), ssh_cfg.ssh_identity_file_path.clone()]
}

fn make_copy_cmd(src_file_name: &str, dst_path: &str, ssh_cfg: &SshCfg) -> RvResult<Command> {
    lazy_static! {
        static ref SPACES_RE: Regex = Regex::new(r"(?P<space>[ ]+)").unwrap();
    }
    let remote_file_path = format!(
        "{}{}{}",
        ssh_cfg.remote_folder_path,
        get_sep(&ssh_cfg.remote_folder_path),
        src_file_name
    );
    let escaped_remote_filepath = SPACES_RE
        .replace_all(&remote_file_path, "'${space}'")
        .to_string();
    let src = format!(
        "{}@{}:{}",
        ssh_cfg.user, ssh_cfg.address, escaped_remote_filepath
    );

    let ssh_args = make_ssh_auth_args(ssh_cfg);
    let src_dst = [src.as_str(), dst_path];
    let scp_cmd_iter = ssh_cfg
        .scp_cmd()
        .iter()
        .chain(ssh_args.iter())
        .map(|s| s.as_str())
        .chain(src_dst.iter().copied());

    make_cmd(scp_cmd_iter)
}

pub fn copy(
    remote_src_file_name: &str,
    dst_path: &str,
    ssh_cfg: &SshCfg,
    override_local: bool,
) -> RvResult<()> {
    let mut cmd = make_copy_cmd(remote_src_file_name, dst_path, ssh_cfg)?;
    println!(" CMD COPY : {:?}", cmd);
    if !Path::new(&dst_path).exists() || override_local {
        cmd.output().map_err(to_rv)?;
    }
    Ok(())
}

fn make_ls_cmd(remote_folder_name: &str, ssh_cfg: &SshCfg) -> RvResult<Command> {
    let ssh_args = make_ssh_auth_args(ssh_cfg);
    let connections_string = format!("{}@{}", ssh_cfg.user, ssh_cfg.address);
    let args = [
        connections_string,
        "ls".to_string(),
        "-1".to_string(),
        remote_folder_name.to_string(),
    ];

    let ssh_cmd_iter = ssh_cfg
        .ssh_cmd()
        .iter()
        .chain(ssh_args.iter())
        .chain(args.iter())
        .map(|s| s.as_str());
    make_cmd(ssh_cmd_iter)
}

fn cmd_output_to_vec(mut cmd: Command) -> RvResult<Vec<String>> {
    let output = cmd.output().map_err(to_rv)?;
    let list = String::from_utf8(output.stdout).map_err(to_rv)?;
    Ok(list.lines().map(|s| s.to_string()).collect::<Vec<_>>())
}

pub fn ssh_ls(remote_folder_name: &str, ssh_cfg: &SshCfg) -> RvResult<Vec<String>> {
    let cmd = make_ls_cmd(remote_folder_name, ssh_cfg)?;
    println!(" CMD LS  :  {:?}", cmd);
    cmd_output_to_vec(cmd)
}

#[cfg(test)]
use crate::cfg::get_default_cfg;
#[test]
fn test_commands() -> RvResult<()> {
    let default_cfg = get_default_cfg();
    let ssh_cfg = default_cfg.ssh_cfg;
    let tmpdir = "testtmpdir with space";
    let file_name = "testfn";
    let dst_path = format!("{tmpdir}/{file_name}");
    let cmd = make_copy_cmd(file_name, &dst_path, &ssh_cfg)?;
    let cmd_string = format!("{:?}", cmd);
    assert_eq!(
        r#""cmd" "/C" "scp" "-i" "local/path" "someuser@73.42.73.42:a/b/c/testfn" "testtmpdir with space/testfn""#,
        cmd_string
    );
    let remote_folder = "/the/remote with space/folder";
    let cmd = make_ls_cmd(remote_folder, &ssh_cfg)?;
    let cmd_string = format!("{:?}", cmd);
    assert_eq!(
        r#""cmd" "/C" "ssh" "-i" "local/path" "someuser@73.42.73.42" "ls" "-1" "/the/remote with space/folder""#,
        cmd_string,
    );
    ssh_ls(remote_folder, &ssh_cfg)?;
    Ok(())
}

#[test]
fn test_outputs() -> RvResult<()> {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "echo", "x"]);
    let v = cmd_output_to_vec(cmd)?;
    assert_eq!(v[0], "x");
    Ok(())
}
