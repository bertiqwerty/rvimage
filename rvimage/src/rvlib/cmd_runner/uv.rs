use std::{path::PathBuf, process::Command};

use rvimage_domain::{RvResult, to_rv};

pub fn install() -> RvResult<()> {
    if cfg!(target_os = "windows") {
        // Windows: try `powershell` and `pwsh`.
        let args = [
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "iwr -useb https://astral.sh/uv/install.ps1 | iex",
        ];
        let mut errors = Vec::new();
        for exe in &["powershell", "pwsh"] {
            match Command::new(exe).args(args).output() {
                Ok(out) if out.status.success() => {
                    tracing::info!("Installed uv using {}.", exe);
                    return Ok(());
                }
                Ok(out) => {
                    let err_msg = String::from_utf8_lossy(&out.stderr);
                    errors.push(format!("{} failed: {}", exe, err_msg.trim()));
                    tracing::warn!(
                        "{} install exited with status {:?}: {}",
                        exe,
                        out.status.code(),
                        err_msg
                    );
                }
                Err(e) => {
                    errors.push(format!("failed to spawn {}: {}", exe, e));
                    tracing::warn!("failed to spawn {}: {}", exe, e);
                }
            }
        }
        return Err(to_rv(format!(
            "Failed to install uv on Windows: {}",
            errors.join("; ")
        )));
    } else {
        // macOS and Linux
        let status = Command::new("sh")
            .args(["-c", "curl -LsSf https://astral.sh/uv/install.sh | sh"])
            .status()
            .map_err(to_rv)?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| to_rv("Failed to install uv on macOS/Linux".to_string()))?;
    }
    Ok(())
}

pub fn find() -> Option<PathBuf> {
    // The installer's target dir is only added to the PATH of new shells.
    let install_dir_uv = dirs::home_dir().map(|h| h.join(".local").join("bin").join("uv"));
    std::iter::once(PathBuf::from("uv"))
        .chain(install_dir_uv)
        .find(|uv| {
            Command::new(uv)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
}

/// Returns the path to uv and installs uv only if it cannot be found.
pub fn ensure() -> RvResult<PathBuf> {
    if let Some(uv) = find() {
        Ok(uv)
    } else {
        tracing::info!("Installing uv...");
        install()?;
        find().ok_or_else(|| to_rv("uv was installed but cannot be found"))
    }
}
