use std::process::Command;

use crate::{
    cfg::SshCfg,
    result::{to_rv, RvError, RvResult},
};

fn get_sep(path_to_foler: &str) -> &'static str {
    if path_to_foler.ends_with("/") {
        ""
    } else {
        "/"
    }
}

fn str_to_cmd(str_cmd: &str, terminal: &str) -> RvResult<Command> {
    let mut terminal_iter = terminal.trim().split_ascii_whitespace();
    let terminal = terminal_iter
        .next()
        .ok_or(RvError::new("couldn't unpack terminal"))?;
    let str_cmd_iter = str_cmd.trim().split_ascii_whitespace();
    let mut cmd = Command::new(terminal);
    cmd.args(terminal_iter.chain(str_cmd_iter));
    Ok(cmd)
}

fn make_ssh_cmd_begin(cmd: &str, ssh_cfg: &SshCfg) -> String {
    format!(
        "{} -i {} {}@{}",
        cmd, ssh_cfg.ssh_identity_file_path, ssh_cfg.user, ssh_cfg.address
    )
}

fn make_copy_cmd(
    file_name: &str,
    local_dst_folder: &str,
    ssh_cfg: &SshCfg,
) -> RvResult<(Command, String)> {
    let src = format!(
        "{}{}{}",
        ssh_cfg.remote_folder_path.as_str(),
        get_sep(&ssh_cfg.remote_folder_path),
        file_name
    );
    let dst = format!(
        "{}{}{}",
        local_dst_folder,
        get_sep(local_dst_folder),
        file_name
    );

    let scp_cmd = format!(
        r#"{}:{} {}"#,
        make_ssh_cmd_begin(ssh_cfg.scp_cmd(), ssh_cfg),
        src,
        dst
    );
    Ok((str_to_cmd(&scp_cmd, ssh_cfg.terminal())?, dst))
}

pub fn copy(
    remote_src_file_name: &str,
    local_dst_folder: &str,
    ssh_cfg: &SshCfg,
) -> RvResult<String> {
    let (mut cmd, dst_file_path) = make_copy_cmd(remote_src_file_name, local_dst_folder, ssh_cfg)?;
    cmd.output().map_err(to_rv)?;
    Ok(dst_file_path)
}

fn make_ls_cmd(remote_folder_name: &str, ssh_cfg: &SshCfg) -> RvResult<Command> {
    let ssh_cmd_begin = make_ssh_cmd_begin(ssh_cfg.ssh_cmd(), ssh_cfg);
    let cmd_str = format!("{} ls -l {}", ssh_cmd_begin, remote_folder_name);
    str_to_cmd(&cmd_str, ssh_cfg.terminal())
}

fn cmd_output_to_vec(mut cmd: Command) -> RvResult<Vec<String>> {
    let output = cmd.output().map_err(to_rv)?;
    let list = String::from_utf8(output.stdout).map_err(to_rv)?;
    Ok(list.lines().map(|s| s.to_string()).collect::<Vec<_>>())
}

pub fn ssh_ls(remote_folder_name: &str, ssh_cfg: &SshCfg) -> RvResult<Vec<String>> {
    let cmd = make_ls_cmd(remote_folder_name, ssh_cfg)?;
    cmd_output_to_vec(cmd)
}

#[cfg(test)]
use crate::cfg::get_default_cfg;
#[test]
fn test_commands() -> RvResult<()> {
    let default_cfg = get_default_cfg();
    let ssh_cfg = default_cfg.ssh_cfg;
    let tmpdir = "testtmpdir";
    let file_name = "testfn";
    let (cmd, dst) = make_copy_cmd(file_name, tmpdir, &ssh_cfg)?;
    let cmd_string = format!("{:?}", cmd);
    assert_eq!(
        r#""cmd" "/C" "scp" "-i" "local/path" "someuser@73.42.73.42:a/b/c/testfn" "testtmpdir/testfn""#,
        cmd_string
    );
    assert_eq!(r#"testtmpdir/testfn"#, dst);
    let remote_folder = "/the/remote/folder";
    let cmd = make_ls_cmd(remote_folder, &ssh_cfg)?;
    let cmd_string = format!("{:?}", cmd);
    assert_eq!(
        r#""cmd" "/C" "ssh" "-i" "local/path" "someuser@73.42.73.42" "ls" "-l" "/the/remote/folder""#,
        cmd_string,
    );
    ssh_ls(remote_folder, &ssh_cfg)?;
    Ok(())
}

#[test]
fn test_outputs() -> RvResult<()> {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C","echo", "x"]);
    let v = cmd_output_to_vec(cmd)?;
    assert_eq!(v[0], "x");
    Ok(())
}
