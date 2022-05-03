use std::marker::PhantomData;
use std::path::Path;

use walkdir::WalkDir;

use crate::cache::{Cache, ReadImageToCache};
use crate::result::{to_rv, AsyncResultImage, ResultImage, RvError, RvResult};
use crate::{format_rverr, util};

#[derive(Clone, Debug)]
pub struct ReadImageFromPath;
impl ReadImageToCache<()> for ReadImageFromPath {
    fn new(_: ()) -> Self {
        Self {}
    }
    fn read_one(&self, path: &str) -> ResultImage {
        Ok(image::io::Reader::open(path)
            .map_err(to_rv)?
            .decode()
            .map_err(|e| format_rverr!("could not decode image {:?}. {:?}", path, e))?
            .into_rgb8())
    }
}

pub fn read_image_paths(path: &str) -> RvResult<Vec<String>> {
    WalkDir::new(path)
        .into_iter()
        .map(|p| p.map_err(to_rv))
        .filter(|p| match p {
            Err(_) => true,
            Ok(p_) => match p_.path().extension() {
                Some(ext) => ext == "png" || ext == "jpg",
                None => false,
            },
        })
        .map(|p| Ok(path_to_str(p?.path())?.to_string()))
        .collect::<RvResult<Vec<String>>>()
}
pub fn to_stem_str(p: &Path) -> RvResult<&str> {
    util::osstr_to_str(p.file_stem())
        .map_err(|e| format_rverr!("could not transform '{:?}' due to '{:?}'", p, e))
}

pub fn to_name_str(p: &Path) -> RvResult<&str> {
    util::osstr_to_str(p.file_name())
        .map_err(|e| format_rverr!("could not transform '{:?}' due to '{:?}'", p, e))
}

pub fn path_to_str(p: &Path) -> RvResult<&str> {
    util::osstr_to_str(Some(p.as_os_str()))
        .map_err(|e| format_rverr!("could not transform '{:?}' due to '{:?}'", p, e))
}

pub trait LoadImageForGui {
    /// read image with index file_selected_idx  
    fn read_image(&mut self, file_selected_idx: usize) -> AsyncResultImage;
    /// get index of selected file
    fn file_selected_idx(&self) -> Option<usize>;
    /// set index of selected file
    fn select_file(&mut self, idx: usize);
    /// list all files in folder
    fn list_file_labels(&self, filter_str: &str) -> RvResult<Vec<(usize, String)>>;
    /// get the user input of a new folder and open it
    fn open_folder(&mut self) -> RvResult<()>;
    /// get the label of the folder to display
    fn folder_label(&self) -> RvResult<String>;
    /// get the label of the selected file to display
    fn file_selected_label(&self) -> RvResult<String>;
}

pub struct Picked {
    pub folder_path: String,
    pub file_paths: Vec<String>,
}

pub trait PickFolder {
    fn pick() -> RvResult<Picked>;
}

pub struct Loader<C, FP, CA>
where
    C: Cache<CA>,
    FP: PickFolder,
{
    file_paths: Vec<String>,
    folder_path: Option<String>,
    file_selected_idx: Option<usize>,
    cache: C,
    pick_phantom: PhantomData<FP>,
    cache_args_phantom: PhantomData<CA>,
}

impl<C, FP, CA> Loader<C, FP, CA>
where
    C: Cache<CA>,
    FP: PickFolder,
{
    pub fn new(cache_args: CA) -> Self {
        Loader {
            file_paths: vec![],
            folder_path: None,
            file_selected_idx: None,
            cache: C::new(cache_args),
            pick_phantom: PhantomData {},
            cache_args_phantom: PhantomData {},
        }
    }
}

impl<C, FP, A> LoadImageForGui for Loader<C, FP, A>
where
    C: Cache<A>,
    FP: PickFolder,
{
    fn read_image(&mut self, file_selected: usize) -> AsyncResultImage {
        self.cache.load_from_cache(file_selected, &self.file_paths)
    }
    fn file_selected_idx(&self) -> Option<usize> {
        self.file_selected_idx
    }
    fn open_folder(&mut self) -> RvResult<()> {
        let picked = FP::pick()?;
        self.folder_path = Some(picked.folder_path);
        self.file_paths = picked.file_paths;
        self.file_selected_idx = None;

        Ok(())
    }
    fn list_file_labels(&self, filter_str: &str) -> RvResult<Vec<(usize, String)>> {
        self.file_paths
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                if filter_str.is_empty() {
                    true
                } else {
                    p.contains(filter_str)
                }
            })
            .map(|(i, p)| Ok((i, to_name_str(Path::new(p))?.to_string())))
            .collect::<RvResult<Vec<_>>>()
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
    fn select_file(&mut self, idx: usize) {
        self.file_selected_idx = Some(idx);
    }
}
#[cfg(test)]
use {crate::cache::NoCache, std::env};

#[cfg(test)]
const TMP_SUBFOLDER: &str = "rvimage_testdata";
#[cfg(test)]
struct TmpFolderPicker;
#[cfg(test)]
impl PickFolder for TmpFolderPicker {
    fn pick() -> RvResult<Picked> {
        let tmpdir = env::temp_dir();
        Ok(Picked {
            folder_path: format!(
                "{}/{}",
                tmpdir
                    .to_str()
                    .ok_or_else(|| format_rverr!("cannot stringify {:?}", tmpdir))?,
                TMP_SUBFOLDER
            ),
            file_paths: vec![],
        })
    }
}
#[cfg(test)]
use {crate::ImageType, std::fs};
#[test]
fn test_folder_reader() -> RvResult<()> {
    let tmp_dir = env::temp_dir().join(TMP_SUBFOLDER);
    match fs::remove_dir_all(&tmp_dir) {
        Ok(_) => (),
        Err(_) => (),
    }
    fs::create_dir(&tmp_dir).map_err(to_rv)?;
    for i in 0..10 {
        let im = ImageType::new(10, 10);
        let out_path = tmp_dir.join(format!("tmpfile_{}.png", i));
        im.save(out_path).unwrap();
    }
    let mut reader = Loader::<NoCache<ReadImageFromPath, ()>, TmpFolderPicker, ()>::new(());
    reader.open_folder()?;
    for (i, (_, label)) in reader.list_file_labels("")?.iter().enumerate() {
        assert_eq!(label[label.len() - 13..], format!("tmpfile_{}.png", i));
    }
    let folder_label = reader.folder_label()?;
    println!("{}", folder_label);
    assert_eq!(
        folder_label[(folder_label.len() - TMP_SUBFOLDER.len())..].to_string(),
        TMP_SUBFOLDER.to_string()
    );
    Ok(())
}
