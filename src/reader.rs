use std::ffi::OsStr;
use std::io::{self, Error, ErrorKind};
use std::path::PathBuf;
use std::{fs, path::Path};

use image::{ImageBuffer, Rgb};

fn read_image_paths(path: &Path) -> io::Result<Vec<PathBuf>> {
    fs::read_dir(path)?
        .into_iter()
        .map(|p| Ok(p?.path()))
        .filter(|p| match p {
            Err(_) => true,
            Ok(p_) => match p_.extension() {
                Some(ext) => ext == "png" || ext == "jpg",
                None => false,
            },
        })
        .collect::<Result<Vec<PathBuf>, Error>>()
}

fn path_to_str<'a, F>(p: &'a Path, func: F) -> io::Result<&'a str>
where
    F: Fn(&'a Path) -> Option<&'a OsStr>,
{
    let find_stem = |p_: &'a Path| func(p_)?.to_str();
    find_stem(p)
        .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("stem of {:?} not found", p)))
}

fn to_stem_str(p: &Path) -> io::Result<&str> {
    path_to_str(p, |p| p.file_stem())
}

fn to_name_str(p: &Path) -> io::Result<&str> {
    path_to_str(p, |p| p.file_name())
}

pub trait ReadImageFiles {
    fn new() -> Self;
    fn next(&mut self);
    fn prev(&mut self);
    fn read_image(&self, file_selected: usize) -> io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>>;
    fn file_selected_idx(&self) -> Option<usize>;
    fn selected_file(&mut self, idx: usize);

    fn list_file_labels(&self) -> io::Result<Vec<String>>;
    fn open_folder(&mut self) -> io::Result<()>;
    fn folder_label(&self) -> io::Result<String>;
    fn file_selected_label(&self) -> io::Result<String>;
}

pub struct FolderReader {
    file_paths: Vec<PathBuf>,
    folder_path: Option<PathBuf>,
    file_selected_idx: Option<usize>,
}

impl ReadImageFiles for FolderReader {
    fn new() -> Self {
        FolderReader {
            file_paths: vec![],
            folder_path: None,
            file_selected_idx: None,
        }
    }
    fn next(&mut self) {
        self.file_selected_idx = self.file_selected_idx.map(|idx| {
            if idx < self.file_paths.len() - 1 {
                idx + 1
            } else {
                idx
            }
        });
    }
    fn prev(&mut self) {
        self.file_selected_idx = self
            .file_selected_idx
            .map(|idx| if idx > 0 { idx - 1 } else { idx });
    }
    fn read_image(&self, file_selected: usize) -> io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        Ok(image::io::Reader::open(&self.file_paths[file_selected])?
            .decode()
            .map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "could not decode image {:?}. {:?}",
                        self.file_paths[file_selected], e
                    ),
                )
            })?
            .into_rgb8())
    }
    fn file_selected_idx(&self) -> Option<usize> {
        self.file_selected_idx
    }
    fn open_folder(&mut self) -> io::Result<()> {
        if let Some(sf) = rfd::FileDialog::new().pick_folder() {
            self.file_paths = read_image_paths(&sf)?;
            self.folder_path = Some(sf);
            self.file_selected_idx = None;
        }
        Ok(())
    }
    fn list_file_labels(&self) -> io::Result<Vec<String>> {
        self.file_paths
            .iter()
            .map(|p| Ok(to_name_str(p)?.to_string()))
            .collect::<io::Result<Vec<String>>>()
    }
    fn folder_label(&self) -> io::Result<String> {
        match &self.folder_path {
            Some(sf) => {
                let last = sf.ancestors().next();
                let one_before_last = sf.ancestors().nth(1);
                match (one_before_last, last) {
                    (Some(obl), Some(l)) => {
                        Ok(format!("{}/{}", to_stem_str(obl)?, to_stem_str(l)?,))
                    }
                    (None, Some(l)) => Ok(to_stem_str(l)?.to_string()),
                    _ => Err(Error::new(
                        ErrorKind::InvalidData,
                        format!("could not convert path {:?} to str", self.folder_path),
                    )),
                }
            }
            None => Ok("no folder selected".to_string()),
        }
    }
    fn file_selected_label(&self) -> io::Result<String> {
        Ok(match self.file_selected_idx {
            Some(idx) => to_name_str(&self.file_paths[idx])?.to_string(),
            None => "no file selected".to_string(),
        })
    }
    fn selected_file(&mut self, idx: usize) {
        self.file_selected_idx = Some(idx);
    }
}
