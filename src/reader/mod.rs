use std::marker::PhantomData;
use std::path::PathBuf;
use std::{fs, path::Path};

use image::{ImageBuffer, Rgb};

use crate::cache::{file_cache::FileCache, Preload};
use crate::result::{to_rv, RvError, RvResult};
use crate::{format_rverr, util};

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

pub struct LocalReader<C = FileCache, FP = DialogPicker>
where
    C: Preload,
    FP: PickFolder,
{
    file_paths: Vec<String>,
    folder_path: Option<String>,
    file_selected_idx: Option<usize>,
    cache: C,
    pick_phantom: PhantomData<FP>,
}

pub fn read_image_from_path(path: &str) -> RvResult<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    Ok(image::io::Reader::open(path)
        .map_err(to_rv)?
        .decode()
        .map_err(|e| format_rverr!("could not decode image {:?}. {:?}", path, e))?
        .into_rgb8())
}

impl<C, FP> ReadImageFiles for LocalReader<C, FP>
where
    C: Preload,
    FP: PickFolder,
{
    fn new() -> Self {
        LocalReader {
            file_paths: vec![],
            folder_path: None,
            file_selected_idx: None,
            cache: C::new(read_image_from_path),
            pick_phantom: PhantomData {},
        }
    }
    fn next(&mut self) {
        self.file_selected_idx = next(self.file_selected_idx, self.file_paths.len());
    }
    fn prev(&mut self) {
        self.file_selected_idx = prev(self.file_selected_idx);
    }
    fn read_image(&mut self, file_selected: usize) -> RvResult<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        self.cache.read_image(file_selected, &self.file_paths)
    }
    fn file_selected_idx(&self) -> Option<usize> {
        self.file_selected_idx
    }
    fn open_folder(&mut self) -> RvResult<()> {
        let sf = FP::pick()?;
        let path_as_string: String = sf
            .to_str()
            .ok_or_else(|| RvError::new("could not transfer path to unicode string"))?
            .to_string();
        self.file_paths = read_image_paths(&path_as_string)?;
        self.folder_path = Some(path_as_string);
        self.file_selected_idx = None;

        Ok(())
    }
    fn list_file_labels(&self) -> RvResult<Vec<String>> {
        self.file_paths
            .iter()
            .map(|p| Ok(to_name_str(Path::new(p))?.to_string()))
            .collect::<RvResult<Vec<String>>>()
    }
    fn folder_label(&self) -> RvResult<String> {
        match &self.folder_path {
            Some(sf) => {
                let folder_path = Path::new(sf);
                let last = folder_path.ancestors().next();
                let one_before_last = folder_path.ancestors().nth(1);
                match (one_before_last, last) {
                    (Some(obl), Some(l)) => {
                        Ok(format!("{}/{}", to_stem_str(obl)?, to_stem_str(l)?,))
                    }
                    (None, Some(l)) => Ok(to_stem_str(l)?.to_string()),
                    _ => Err(format_rverr!(
                        "could not convert path {:?} to str",
                        self.folder_path
                    )),
                }
            }
            None => Ok("no folder selected".to_string()),
        }
    }
    fn file_selected_label(&self) -> RvResult<String> {
        Ok(match self.file_selected_idx {
            Some(idx) => to_name_str(Path::new(&self.file_paths[idx]))?.to_string(),
            None => "no file selected".to_string(),
        })
    }
    fn selected_file(&mut self, idx: usize) {
        self.file_selected_idx = Some(idx);
    }
}

#[cfg(test)]
use {crate::cache::NoCache, std::env};
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
