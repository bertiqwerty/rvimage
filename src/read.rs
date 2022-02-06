use std::fs;
use std::io::Error;
use std::path::PathBuf;

use image::{ImageBuffer, Rgb};

pub trait ReadImageFiles {
    fn new() -> Self;
    fn next(&mut self);
    fn prev(&mut self);
    fn read_image(&self, file_selected: usize) -> ImageBuffer<Rgb<u8>, Vec<u8>>;
    fn file_selected_idx(&self) -> Option<usize>;
    fn selected_file(&mut self, idx: usize);
    
    fn list_file_paths(&self) -> Vec<PathBuf>;
    fn open_folder(&mut self) -> Vec<PathBuf>;
    fn folder_path(&self) -> Option<PathBuf>;
    fn file_selected(&self) -> Option<PathBuf>;
}

fn read_images_paths(path: &PathBuf) -> Result<Vec<PathBuf>, Error> {
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
    fn open_folder(&mut self) -> Vec<PathBuf> {
        if let Some(sf) = rfd::FileDialog::new().pick_folder() {
            let image_paths = read_images_paths(&sf);
            match image_paths {
                Ok(ip) => self.file_paths = ip,
                Err(e) => println!("{:?}", e),
            }
            self.folder_path = Some(sf);
            self.file_paths.clone()
        } else {
            vec![]
        }
    }
    fn list_file_paths(&self) -> Vec<PathBuf> {
        self.file_paths.clone()
    }
    fn folder_path(&self) -> Option<PathBuf> {
        self.folder_path.clone()
    }
    fn file_selected(&self) -> Option<PathBuf> {
        self.file_selected_idx.map(|idx| self.file_paths[idx].clone())
    }
    fn selected_file(&mut self, idx: usize) {
        self.file_selected_idx = Some(idx);
    }
}
