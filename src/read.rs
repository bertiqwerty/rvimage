use std::io::Error;
use std::path::PathBuf;
use std::{fs, path::Path};

use image::{ImageBuffer, Rgb};

pub trait ReadImageFiles {
    fn new() -> Self;
    fn next(&mut self);
    fn prev(&mut self);
    fn read_image(&self, file_selected: usize) -> ImageBuffer<Rgb<u8>, Vec<u8>>;
    fn file_selected_idx(&self) -> Option<usize>;
    fn selected_file(&mut self, idx: usize);

    fn list_file_labels(&self) -> Vec<String>;
    fn open_folder(&mut self);
    fn folder_label(&self) -> String;
    fn file_selected_label(&self) -> String;
}

fn read_image_paths(path: &PathBuf) -> Result<Vec<PathBuf>, Error> {
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

fn to_stem_str<'a>(x: &'a Path) -> &'a str {
    x.file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
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
    fn read_image(&self, file_selected: usize) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        image::io::Reader::open(&self.file_paths[file_selected])
            .unwrap()
            .decode()
            .unwrap()
            .into_rgb8()
    }
    fn file_selected_idx(&self) -> Option<usize> {
        self.file_selected_idx
    }
    fn open_folder(&mut self) {
        if let Some(sf) = rfd::FileDialog::new().pick_folder() {
            let image_paths = read_image_paths(&sf);
            match image_paths {
                Ok(ip) => self.file_paths = ip,
                Err(e) => println!("{:?}", e),
            }
            self.folder_path = Some(sf);
            self.file_selected_idx = None;
        } 
    }
    fn list_file_labels(&self) -> Vec<String> {
        self.file_paths
            .iter()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<String>>()
    }
    fn folder_label(&self) -> String {
        match &self.folder_path {
            Some(sf) => {
                let last = sf.ancestors().next();
                let one_before_last = sf.ancestors().nth(1);
                match (one_before_last, last) {
                    (Some(obl), Some(l)) => {
                        format!("{}/{}", to_stem_str(obl), to_stem_str(l),)
                    }
                    (None, Some(l)) => to_stem_str(l).to_string(),
                    _ => "could not convert path to str".to_string(),
                }
            }
            None => "no folder selected".to_string(),
        }
    }
    fn file_selected_label(&self) -> String {
        match self.file_selected_idx {
            Some(idx) => self.file_paths[idx]
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string(),
            None => "no file selected".to_string(),
        }
    }
    fn selected_file(&mut self, idx: usize) {
        self.file_selected_idx = Some(idx);
    }
}
