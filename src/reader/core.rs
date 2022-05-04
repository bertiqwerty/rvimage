use std::marker::PhantomData;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::cache::Cache;
use crate::result::{AsyncResultImage, RvError, RvResult};
use crate::{format_rverr, util};

#[derive(Clone)]
pub struct CloneDummy;

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
    cache_args: CA,
    n_cache_recreations: usize,
    pick_phantom: PhantomData<FP>,
}

impl<C, FP, CA> Loader<C, FP, CA>
where
    C: Cache<CA>,
    CA: Clone,
    CA: Clone,
    FP: PickFolder,
{
    pub fn new(cache_args: CA, n_cache_recreations: usize) -> RvResult<Self> {
        Ok(Loader {
            file_paths: vec![],
            folder_path: None,
            file_selected_idx: None,
            cache: C::new(cache_args.clone())?,
            cache_args: cache_args,
            n_cache_recreations: n_cache_recreations,
            pick_phantom: PhantomData {},
        })
    }
}

impl<C, FP, CA> LoadImageForGui for Loader<C, FP, CA>
where
    CA: Clone,
    C: Cache<CA>,
    FP: PickFolder,
{
    fn read_image(&mut self, selected_file_idx: usize) -> AsyncResultImage {
        let mut loaded = self
            .cache
            .load_from_cache(selected_file_idx, &self.file_paths);
        let mut counter = 0usize;
        while let Err(e) = loaded {
            println!(
                "recreating cache ({}/{}), {:?}",
                counter + 1,
                self.n_cache_recreations,
                e
            );
            thread::sleep(Duration::from_millis(500));
            self.cache = C::new(self.cache_args.clone())?;
            loaded = self
                .cache
                .load_from_cache(selected_file_idx, &self.file_paths);
            if counter == self.n_cache_recreations {
                println!("max recreations (={}) reached.", counter);
                return loaded;
            }
            counter += 1;
        }
        loaded
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
