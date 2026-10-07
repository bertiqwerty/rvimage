use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Child, Command},
};

use rvimage_domain::{RvResult, rverr, to_rv};

pub mod uv;

pub const NO_ENV: Option<&HashMap<&str, &str>> = None;

fn build_cmd<K, V>(
    cmd: &str,
    extra_args: &[String],
    prj_path: &Path,
    install_uv: bool,
    working_dir: Option<&Path>,
    env_vars: Option<&HashMap<K, V>>,
) -> RvResult<Command>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut parts = cmd.split_whitespace();
    let program = parts.next().ok_or_else(|| rverr!("command is empty"))?;
    let working_dir =
        working_dir.or_else(|| prj_path.parent().filter(|p| !p.as_os_str().is_empty()));
    let local_program = working_dir
        .map(|wd| wd.join(program))
        .filter(|p| p.is_file());
    let program = if let Some(local_program) = local_program {
        // Unix would resolve a relative program against working_dir a second time.
        std::path::absolute(local_program).map_err(to_rv)?
    } else if install_uv && program == "uv" {
        uv::ensure()?
    } else {
        PathBuf::from(program)
    };
    let mut command = Command::new(program);
    command.args(parts).args(extra_args);
    if let Some(working_dir) = working_dir {
        command.current_dir(working_dir);
    }
    if let Some(env_vars) = env_vars {
        command.envs(env_vars.iter().map(|(k, v)| (k.as_ref(), v.as_ref())));
    }
    Ok(command)
}

/// Runs the whitespace-separated `cmd` followed by `extra_args` until it exits and returns its trimmed stdout.
/// The program is resolved relative to `working_dir`, which defaults to the project folder.
/// With `install_uv`, a program `uv` is installed if missing.
pub fn run_cmd<K, V>(
    cmd: &str,
    extra_args: &[String],
    prj_path: &Path,
    install_uv: bool,
    working_dir: Option<&Path>,
    env_vars: Option<&HashMap<K, V>>,
) -> RvResult<String>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    tracing::info!("running command '{}'...", cmd);
    let output = build_cmd(cmd, extra_args, prj_path, install_uv, working_dir, env_vars)?
        .output()
        .map_err(to_rv)?;
    if !output.status.success() {
        return Err(rverr!(
            "command '{}' failed with {}: {}",
            cmd,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8(output.stdout).map_err(to_rv)?;
    tracing::info!("...done running command.");
    Ok(stdout.trim().to_string())
}

/// Spawns `cmd` followed by `extra_args` with inherited stdio and returns the running child.
/// The program is resolved as in [`run_cmd`].
pub fn trigger_cmd<K, V>(
    cmd: &str,
    extra_args: &[String],
    prj_path: &Path,
    install_uv: bool,
    working_dir: Option<&Path>,
    env_vars: Option<&HashMap<K, V>>,
) -> RvResult<Child>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    tracing::info!("triggering command '{}'...", cmd);
    build_cmd(cmd, extra_args, prj_path, install_uv, working_dir, env_vars)?
        .spawn()
        .map_err(to_rv)
}
