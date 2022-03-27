use std::{marker::PhantomData, path::Path};

use image::{ImageBuffer, Rgb};

use crate::{
    cache::{file_cache::FileCache, Preload},
    format_rverr,
    result::{to_rv, RvError, RvResult},
};

use super::{
    next, prev, read_image_paths, to_name_str, to_stem_str, DialogPicker, PickFolder,
    ReadImageFiles,
};

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
impl<C, FP> LocalReader<C, FP>
where
    C: Preload,
    FP: PickFolder,
{
    pub fn new() -> Self {
        LocalReader {
            file_paths: vec![],
            folder_path: None,
            file_selected_idx: None,
            cache: C::new(read_image_from_path),
            pick_phantom: PhantomData {},
        }
    }
}
impl<C, FP> ReadImageFiles for LocalReader<C, FP>
where
    C: Preload,
    FP: PickFolder,
{
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
    fn select_file(&mut self, idx: usize) {
        self.file_selected_idx = Some(idx);
    }
}
