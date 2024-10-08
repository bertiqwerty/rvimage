use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::cache::Cache;
use crate::file_util::PathPair;
use crate::paths_selector::PathsSelector;
use crate::types::AsyncResultImage;
use rvimage_domain::RvResult;

pub const SUPPORTED_EXTENSIONS: [&str; 10] = [
    ".PNG", ".png", ".JPG", ".jpg", ".JPEG", ".jpeg", ".TIF", ".tif", ".TIFF", ".tiff",
];

#[derive(Clone)]
pub struct CloneDummy;

/// All [`Loader`](Loader) structs with their different generic parameters implement this trait
/// such that the loader can be created dynamically based on the config.
pub trait LoadImageForGui {
    /// read image with index file_selected_idx  
    fn read_image(
        &mut self,
        file_selected_idx: usize,
        abs_file_paths: &[&str],
        reload: bool,
    ) -> AsyncResultImage;
    /// get the user input of a new folder and open it
    fn open_folder(&self, abs_folder_path: &str, prj_path: &Path) -> RvResult<PathsSelector>;
}

pub struct Loader<C, CA>
where
    C: Cache<CA>,
{
    cache: C,
    cache_args: CA,
    n_cache_recreations: usize,
}

impl<C, CA> Loader<C, CA>
where
    C: Cache<CA>,
    CA: Clone,
{
    pub fn new(cache_args: CA, n_cache_recreations: usize) -> RvResult<Self> {
        Ok(Loader {
            cache: C::new(cache_args.clone())?,
            cache_args,
            n_cache_recreations,
        })
    }
}

impl<C, CA> LoadImageForGui for Loader<C, CA>
where
    C: Cache<CA>,
    CA: Clone,
{
    fn read_image(
        &mut self,
        selected_file_idx: usize,
        abs_file_paths: &[&str],
        reload: bool,
    ) -> AsyncResultImage {
        let mut loaded = self
            .cache
            .load_from_cache(selected_file_idx, abs_file_paths, reload);
        let mut counter = 0usize;
        while let Err(e) = loaded {
            tracing::info!(
                "recreating cache ({}/{}), {:?}",
                counter + 1,
                self.n_cache_recreations,
                e
            );
            thread::sleep(Duration::from_millis(500));
            self.cache = C::new(self.cache_args.clone())?;
            loaded = self
                .cache
                .load_from_cache(selected_file_idx, abs_file_paths, reload);
            if counter == self.n_cache_recreations {
                tracing::info!("max recreations (={counter}) reached.");
                return loaded;
            }
            counter += 1;
        }
        loaded
    }

    fn open_folder(&self, abs_folder_path: &str, prj_path: &Path) -> RvResult<PathsSelector> {
        let file_paths = self
            .cache
            .ls(abs_folder_path)?
            .iter()
            .map(|p| PathPair::new(p.to_string(), prj_path))
            .collect::<Vec<_>>();

        PathsSelector::new(file_paths, Some(abs_folder_path.to_string()))
    }
}
