use std::marker::PhantomData;
use std::thread;
use std::time::Duration;

use crate::cache::Cache;
use crate::paths_selector::PathsSelector;
use crate::result::RvResult;
use crate::types::AsyncResultImage;

pub const SUPPORTED_EXTENSIONS: [&str; 4] = [".png", ".jpg", ".tif", ".tiff"];

#[derive(Clone)]
pub struct CloneDummy;

pub trait LoadImageForGui {
    /// read image with index file_selected_idx  
    fn read_image(&mut self, file_selected_idx: usize, file_paths: &[String]) -> AsyncResultImage;
    /// get the user input of a new folder and open it
    fn open_folder(&self) -> RvResult<PathsSelector>;
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
            cache: C::new(cache_args.clone())?,
            cache_args,
            n_cache_recreations,
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
    fn read_image(&mut self, selected_file_idx: usize, file_paths: &[String]) -> AsyncResultImage {
        let mut loaded = self.cache.load_from_cache(selected_file_idx, file_paths);
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
            loaded = self.cache.load_from_cache(selected_file_idx, file_paths);
            if counter == self.n_cache_recreations {
                println!("max recreations (={}) reached.", counter);
                return loaded;
            }
            counter += 1;
        }
        loaded
    }
    fn open_folder(&self) -> RvResult<PathsSelector> {
        let picked = FP::pick()?;

        PathsSelector::new(picked.file_paths, Some(picked.folder_path))
    }
}
