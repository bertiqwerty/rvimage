use std::path::PathBuf;
use std::{fs, path::Path};

use image::{ImageBuffer, Rgb};

use crate::result::{to_rv, RvError, RvResult};
use crate::util;

pub mod from_cfg;
pub mod local_reader;
pub mod scp_reader;

fn read_image_paths(path: &str) -> RvResult<Vec<String>> {
    fs::read_dir(path)
        .map_err(to_rv)?
        .into_iter()
        .map(|p| Ok(p.map_err(to_rv)?.path()))
        .filter(|p: &RvResult<PathBuf>| match p {
            Err(_) => true,
            Ok(p_) => match p_.extension() {
                Some(ext) => ext == "png" || ext == "jpg",
                None => false,
            },
        })
        .map(|p| Ok(path_to_str(&p?)?.to_string()))
        .collect::<RvResult<Vec<String>>>()
}

fn to_stem_str(p: &Path) -> RvResult<&str> {
    util::osstr_to_str(p.file_stem()).map_err(to_rv)
}

fn to_name_str(p: &Path) -> RvResult<&str> {
    util::osstr_to_str(p.file_name()).map_err(to_rv)
}

fn path_to_str(p: &Path) -> RvResult<&str> {
    util::osstr_to_str(Some(p.as_os_str())).map_err(to_rv)
}

pub trait ReadImageFiles {
    fn new() -> Self;
    fn next(&mut self);
    fn prev(&mut self);
    fn read_image(&mut self, file_selected_idx: usize) -> RvResult<ImageBuffer<Rgb<u8>, Vec<u8>>>;
    fn file_selected_idx(&self) -> Option<usize>;
    fn selected_file(&mut self, idx: usize);
    fn list_file_labels(&self) -> RvResult<Vec<String>>;
    fn open_folder(&mut self) -> RvResult<()>;
    fn folder_label(&self) -> RvResult<String>;
    fn file_selected_label(&self) -> RvResult<String>;
}

pub fn next(file_selected_idx: Option<usize>, files_len: usize) -> Option<usize> {
    file_selected_idx.map(|idx| if idx < files_len - 1 { idx + 1 } else { idx })
}

pub fn prev(file_selected_idx: Option<usize>) -> Option<usize> {
    file_selected_idx.map(|idx| if idx > 0 { idx - 1 } else { idx })
}

pub trait PickFolder {
    fn pick() -> RvResult<PathBuf>;
}

pub struct DialogPicker;
impl PickFolder for DialogPicker {
    fn pick() -> RvResult<PathBuf> {
        rfd::FileDialog::new()
            .pick_folder()
            .ok_or_else(|| RvError::new("Could not pick folder."))
    }
}

#[cfg(test)]
use {
    crate::{cache::NoCache, reader::local_reader::LocalReader},
    std::env,
};
#[cfg(test)]
const TMP_SUBFOLDER: &str = "rimview_testdata";
#[cfg(test)]
struct TmpFolderPicker;
#[cfg(test)]
impl PickFolder for TmpFolderPicker {
    fn pick() -> RvResult<PathBuf> {
        Ok(env::temp_dir().join(TMP_SUBFOLDER))
    }
}

#[test]
fn test_folder_reader() -> RvResult<()> {
    let tmp_dir = env::temp_dir().join(TMP_SUBFOLDER);
    match fs::remove_dir_all(&tmp_dir) {
        Ok(_) => (),
        Err(_) => (),
    }
    fs::create_dir(&tmp_dir).map_err(to_rv)?;
    for i in 0..10 {
        let im = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(10, 10);
        let out_path = tmp_dir.join(format!("tmpfile_{}.png", i));
        im.save(out_path).unwrap();
    }

    let mut reader = LocalReader::<NoCache, TmpFolderPicker>::new();
    reader.open_folder()?;
    for (i, label) in reader.list_file_labels()?.iter().enumerate() {
        assert_eq!(label[label.len() - 13..], format!("tmpfile_{}.png", i));
    }
    let folder_label = reader.folder_label()?;
    assert_eq!(
        folder_label[(folder_label.len() - TMP_SUBFOLDER.len())..].to_string(),
        TMP_SUBFOLDER.to_string()
    );
    Ok(())
}
