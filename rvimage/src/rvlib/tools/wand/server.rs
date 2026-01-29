use rvimage_domain::{RvResult, to_rv};
use std::fmt::Debug;
use std::fs;
use std::path::Path;
use std::process::{Child, Command};

use crate::cfg::CmdServerSrc;
use crate::file_util;
use crate::result::trace_ok_err;

fn install_uv() -> RvResult<()> {
    if cfg!(target_os = "windows") {
        // Windows
        let status = Command::new("powershell")
            .args(["-Command", "irm https://astral.sh/uv/install.ps1 | iex"])
            .status()
            .map_err(to_rv)?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| to_rv("Failed to install uv on Windows".to_string()))?;
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

pub trait WandServer: Debug + Send + Sync {
    fn cleanup_server(&mut self) -> RvResult<()>;
    fn start_server(&mut self, prj_path: &Path) -> RvResult<()>;
    fn stop_server(&mut self) -> RvResult<()>;
}

/// A Wand server implementation that runs an external command to start the server.
///
/// # Fields
/// - `src`: Where to get the executable/source code for the server
/// - `setup_cmd`: Command to set up and run the server
/// - `setup_args`: Arguments for the setup command
/// - `local_folder`: Local folder to extract the source code, default is $HOME/wand_server
/// - `child`: The child process running the server
///
#[derive(Debug)]
pub struct CmdServer {
    src: CmdServerSrc,
    additional_files: Vec<String>,
    setup_cmd: String,
    setup_args: Vec<String>,
    local_base_folder: String,
    install_uv: bool,
    child: Option<Child>,
}

impl CmdServer {
    pub fn new(
        src: CmdServerSrc,
        additional_files: Vec<String>,
        setup_cmd: String,
        setup_args: Vec<String>,
        install_uv: bool,
        local_folder: String,
    ) -> Self {
        CmdServer {
            src,
            additional_files,
            setup_cmd,
            setup_args,
            local_base_folder: local_folder,
            install_uv,
            child: None,
        }
    }
}

impl WandServer for CmdServer {
    fn start_server(&mut self, prj_path: &Path) -> RvResult<()> {
        if self.install_uv {
            tracing::info!("Installing uv...");
            install_uv()?;
        }

        let local_repo_path =
            Path::new(&self.local_base_folder).join(self.src.relative_working_dir());

        // Check if the folder already exists
        if local_repo_path.exists() && local_repo_path.read_dir().map_err(to_rv)?.next().is_some() {
            tracing::info!(
                "Local folder {} already exists and is not empty. Skipping download.",
                self.local_base_folder
            );
        } else {
            tracing::info!(
                "Copying or downloading wand server and unzipping {:?} to {}...",
                self.src,
                self.local_base_folder
            );
            self.src
                .put_to_dst(prj_path, Path::new(&self.local_base_folder))?;
        }
        for af in &self.additional_files {
            let src_path = Path::new(af);
            let src_path = file_util::relative_to_prj_path(prj_path, src_path)?;
            let file_name = src_path
                .file_name()
                .ok_or_else(|| to_rv(format!("Invalid additional file path: {}", af)))?;
            let dest_path = local_repo_path.join(file_name);
            if !dest_path.exists() {
                tracing::info!(
                    "Copying additional file {} to {}...",
                    af,
                    dest_path.display()
                );
                fs::copy(src_path, dest_path).map_err(to_rv)?;
            }
        }
        let churdir = format!(
            "{}/{}",
            self.local_base_folder,
            self.src.relative_working_dir()
        );
        tracing::info!("Starting wand server from folder {churdir}...");

        let child = Command::new(&self.setup_cmd)
            .args(&self.setup_args)
            .env("PYTHONPATH", ".")
            .current_dir(churdir)
            .spawn()
            .map_err(to_rv)?;
        self.child = Some(child);
        tracing::info!("Wand server up and running.");
        Ok(())
    }

    fn cleanup_server(&mut self) -> RvResult<()> {
        // Remove the local repo folder
        if Path::new(&self.local_base_folder).exists() {
            tracing::info!("Removing local folder {}...", self.local_base_folder);
            fs::remove_dir_all(&self.local_base_folder).map_err(to_rv)?;
        }
        trace_ok_err(self.stop_server());
        Ok(())
    }

    fn stop_server(&mut self) -> RvResult<()> {
        if let Some(mut child) = self.child.take() {
            child.kill().map_err(to_rv)?;
        }
        Ok(())
    }
}
impl Drop for CmdServer {
    fn drop(&mut self) {
        let _ = self.stop_server();
    }
}
