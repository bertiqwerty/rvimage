use rvimage_domain::{RvResult, to_rv};
use std::fmt::Debug;
use std::io::Cursor;
use std::path::Path;
use std::process::{Child, Command};
use zip;

use crate::result::trace_ok_err;

pub trait WandServer: Debug + Send + Sync {
    fn cleanup_server(&mut self) -> RvResult<()>;
    fn start_server(&mut self) -> RvResult<()>;
    fn stop_server(&mut self) -> RvResult<()>;
}

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

/// A Wand server implementation using FastAPI
///
/// # Fields
/// - `srczip_archive_path_or_url`: Path or URL to the source zip archive
/// - `setup_cmd`: Command to set up and run the server
/// - `setup_args`: Arguments for the setup command
/// - `local_folder`: Local folder to extract the source code, default is $HOME/wand_server
/// - `child`: The child process running the server
///
#[derive(Debug)]
pub struct FastAPI {
    srczip_archive_path_or_url: String,
    setup_cmd: String,
    setup_args: Vec<String>,
    local_folder: String,
    child: Option<Child>,
}

impl FastAPI {
    pub fn new(
        srczip_archive_path_or_url: String,
        setup_cmd: String,
        setup_args: Vec<String>,
        local_folder: String,
    ) -> Self {
        FastAPI {
            srczip_archive_path_or_url,
            setup_cmd,
            setup_args,
            local_folder,
            child: None,
        }
    }
}

fn copy_or_dl_and_unzip(src_zip: &str, dst_folder: &str) -> RvResult<()> {
    if Path::new(src_zip).exists() {
        // Local file
        let file = std::fs::File::open(src_zip).map_err(to_rv)?;
        let mut archive = zip::ZipArchive::new(file).map_err(to_rv)?;
        archive.extract(dst_folder).map_err(to_rv)
    } else {
        // URL
        let response = reqwest::blocking::get(src_zip).map_err(to_rv)?;
        let content = response.bytes().map_err(to_rv)?;
        let des = Cursor::new(content);
        let mut archive = zip::ZipArchive::new(des).map_err(to_rv)?;
        archive.extract(dst_folder).map_err(to_rv)
    }
}

impl WandServer for FastAPI {
    fn start_server(&mut self) -> RvResult<()> {
        install_uv()?;

        let local_repo_path = Path::new(&self.local_folder);

        // Check if the folder already exists
        if local_repo_path.exists() && local_repo_path.read_dir().map_err(to_rv)?.next().is_some() {
            tracing::info!(
                "Local folder {} already exists and is not empty. Skipping download.",
                self.local_folder
            );
        } else {
            tracing::info!(
                "Copying or downloading wand server and unzipping {} to {}...",
                self.srczip_archive_path_or_url,
                self.local_folder
            );
            copy_or_dl_and_unzip(&self.srczip_archive_path_or_url, &self.local_folder)?;
        }
        let churdir = format!(
            "{}/{}",
            self.local_folder,
            self.srczip_archive_path_or_url
                .trim_end_matches(".zip")
                .rsplit('/')
                .next()
                .unwrap_or("")
        );
        tracing::info!("Starting wand server from folder {churdir}...");

        let child = Command::new(&self.setup_cmd)
            .args(&self.setup_args)
            .env("PYTHONPATH", ".")
            .current_dir(churdir)
            .spawn()
            .map_err(to_rv)?;
        self.child = Some(child);
        Ok(())
    }

    fn cleanup_server(&mut self) -> RvResult<()> {
        // Remove the local repo folder
        if Path::new(&self.local_folder).exists() {
            tracing::info!("Removing local folder {}...", self.local_folder);
            std::fs::remove_dir_all(&self.local_folder).map_err(to_rv)?;
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
impl Drop for FastAPI {
    fn drop(&mut self) {
        let _ = self.stop_server();
    }
}
