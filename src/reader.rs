use std::ffi::OsStr;
use std::io::{self, Error, ErrorKind};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::{fs, path::Path};

use image::{ImageBuffer, Rgb};

use crate::cache::{NoCache, Preload};

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
    fn read_image(&self, file_selected_idx: usize) -> io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>>;
    fn file_selected_idx(&self) -> Option<usize>;
    fn selected_file(&mut self, idx: usize);

    fn list_file_labels(&self) -> io::Result<Vec<String>>;
    fn open_folder(&mut self) -> io::Result<()>;
    fn folder_label(&self) -> io::Result<String>;
    fn file_selected_label(&self) -> io::Result<String>;
}

pub fn next(file_selected_idx: Option<usize>, files_len: usize) -> Option<usize> {
    file_selected_idx.map(|idx| if idx < files_len - 1 { idx + 1 } else { idx })
}

pub fn prev(file_selected_idx: Option<usize>) -> Option<usize> {
    file_selected_idx.map(|idx| if idx > 0 { idx - 1 } else { idx })
}

pub trait PickFolder {
    fn pick() -> Option<PathBuf>;
}

pub struct DialogPicker;
impl PickFolder for DialogPicker{
    fn pick() -> Option<PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }
}

pub struct FolderReader<Cache = NoCache, FolderPicker = DialogPicker>
where
    Cache: Preload,
    FolderPicker: PickFolder
{
    file_paths: Vec<PathBuf>,
    folder_path: Option<PathBuf>,
    file_selected_idx: Option<usize>,
    cache: Cache,
    pick_phantom: PhantomData<FolderPicker>
}

pub fn read_image(path: &Path) -> io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    Ok(image::io::Reader::open(path)?
        .decode()
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("could not decode image {:?}. {:?}", path, e),
            )
        })?
        .into_rgb8())
}

impl<Cache, FolderPicker> ReadImageFiles for FolderReader<Cache, FolderPicker>
where
    Cache: Preload,
    FolderPicker: PickFolder
{
    fn new() -> Self {
        FolderReader {
            file_paths: vec![],
            folder_path: None,
            file_selected_idx: None,
            cache: Cache::new(read_image),
            pick_phantom: PhantomData{}
        }
    }
    fn next(&mut self) {
        self.file_selected_idx = next(self.file_selected_idx, self.file_paths.len());
    }
    fn prev(&mut self) {
        self.file_selected_idx = prev(self.file_selected_idx);
    }
    fn read_image(&self, file_selected: usize) -> io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        self.cache.read_image(&self.file_paths[file_selected])
    }
    fn file_selected_idx(&self) -> Option<usize> {
        self.file_selected_idx
    }
    fn open_folder(&mut self) -> io::Result<()> {
        if let Some(sf) = FolderPicker::pick() {
            self.file_paths = read_image_paths(&sf)?;
            self.folder_path = Some(sf);
            self.file_selected_idx = None;
            self.cache.preload_images(&self.file_paths);
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


#[cfg(test)]
use std::env;
#[cfg(test)]
const TMP_SUBFOLDER: &str = "rimview_testdata";
#[cfg(test)]
struct TmpFolderPicker;
#[cfg(test)]
impl PickFolder for TmpFolderPicker {
    fn pick() -> Option<PathBuf> {
        Some(env::temp_dir().join(TMP_SUBFOLDER))
    }
}

#[test]
fn test_folder_reader() -> io::Result<()> {
    let tmp_dir = env::temp_dir().join(TMP_SUBFOLDER);
    match fs::remove_dir_all(&tmp_dir) {
        Ok(_) => (),
        Err(_) => ()
    }
    fs::create_dir(&tmp_dir)?;
    for i in 0..10 {
        let im = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(10, 10);
        let out_path = tmp_dir.join(format!("tmpfile_{}.png", i));
        im.save(out_path).unwrap();
    }
    
    let mut reader = FolderReader::<NoCache, TmpFolderPicker>::new();
    reader.open_folder()?;
    for (i, label) in reader.list_file_labels()?.iter().enumerate() {
        assert_eq!(label[label.len() - 13..], format!("tmpfile_{}.png", i));
    }
    let folder_label = reader.folder_label()?;
    assert_eq!(folder_label[(folder_label.len() - TMP_SUBFOLDER.len())..].to_string(), TMP_SUBFOLDER.to_string());
    Ok(())
}