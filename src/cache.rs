use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
};

use image::{ImageBuffer, Rgb};

use crate::{
    format_rverr,
    result::{to_rv, RvError, RvResult},
    threadpool::ThreadPool,
    util,
};

type ResultImage = RvResult<ImageBuffer<Rgb<u8>, Vec<u8>>>;

type DefaultReader = fn(&str) -> ResultImage;

pub trait Preload<F = DefaultReader>
where
    F: Fn(&str) -> ResultImage,
{
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage;
    fn new(reader: F) -> Self;
}

pub struct NoCache<F = DefaultReader>
where
    F: Fn(&str) -> ResultImage,
{
    reader: F,
}

impl<F> Preload<F> for NoCache<F>
where
    F: Fn(&str) -> ResultImage,
{
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage {
        (self.reader)(&files[selected_file_idx])
    }
    fn new(reader: F) -> Self {
        Self { reader }
    }
}

pub fn filename_in_tmpdir(path: &str) -> RvResult<String> {
    let path = PathBuf::from_str(path).unwrap();
    let fname = util::osstr_to_str(path.file_name()).map_err(to_rv)?;
    std::env::temp_dir()
        .join(fname)
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format_rverr!("could not transform {:?} to &str", fname))
}

fn copy<F>(path_or_url: &str, reader: F, target: &str) -> RvResult<()>
where
    F: Fn(&str) -> ResultImage,
{
    let im = reader(path_or_url).map_err(to_rv)?;
    im.save(&target)
        .map_err(|e| format_rverr!("could not save image to {:?}. {}", target, e.to_string()))?;
    Ok(())
}

pub trait ReaderType: Fn(&str) -> ResultImage + Send + Sync + Clone + 'static {}
impl<T: Fn(&str) -> ResultImage + Send + Sync + Clone + 'static> ReaderType for T {}

fn preload<F: ReaderType>(
    files: &[String],
    tp: &mut ThreadPool<RvResult<String>>,
    cache: &HashMap<String, ThreadResult>,
    reader: &F,
) -> RvResult<HashMap<String, ThreadResult>> {
    files
        .iter()
        .filter(|file| !cache.contains_key(*file))
        .map(|file| {
            let tmp_file = filename_in_tmpdir(&file)?;
            let file_for_thread = file.clone();
            let reader = reader.clone();
            let job = Box::new(move || match copy(&file_for_thread, reader, &tmp_file) {
                Ok(_) => Ok(tmp_file),
                Err(e) => Err(e),
            });
            Ok((file.clone(), ThreadResult::Running(tp.apply(job)?)))
        })
        .collect::<RvResult<HashMap<_, _>>>()
}

enum ThreadResult {
    Running(usize),
    Ok(String),
}
pub struct FileCache<F = DefaultReader>
where
    F: ReaderType,
{
    reader: F,
    cached_paths: HashMap<String, ThreadResult>,
    half_n_images: usize,
    tp: ThreadPool<RvResult<String>>,
}
impl<F> Preload<F> for FileCache<F>
where
    F: ReaderType,
{
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage {
        let start_idx = if selected_file_idx > self.half_n_images {
            selected_file_idx - self.half_n_images
        } else {
            0
        };
        let end_idx = if selected_file_idx < files.len() - self.half_n_images {
            selected_file_idx + self.half_n_images
        } else {
            files.len() - 1
        };
        let files_to_preload = &files[start_idx..end_idx];

        let cache = preload(
            files_to_preload,
            &mut self.tp,
            &self.cached_paths,
            &self.reader,
        )?;
        // update cache
        for elt in cache.into_iter() {
            let (file, th_res) = elt;
            self.cached_paths.insert(file, th_res);
        }
        let selected_file = &files[selected_file_idx];
        let selected_file_state = &self.cached_paths[selected_file];
        match selected_file_state {
            ThreadResult::Ok(path_in_cache) => (self.reader)(path_in_cache),
            ThreadResult::Running(job_id) => {
                let path_in_cache = self
                    .tp
                    .poll(*job_id)
                    .ok_or(format_rverr!("didn't find job {}", job_id))??;
                let res = (self.reader)(&path_in_cache);
                *self.cached_paths.get_mut(selected_file).unwrap() =
                    ThreadResult::Ok(path_in_cache);
                res
            }
        }
    }
    fn new(reader: F) -> Self {
        let half_n_images = 5;
        let tp = ThreadPool::new(half_n_images);
        Self {
            reader,
            cached_paths: HashMap::new(),
            half_n_images: half_n_images,
            tp,
        }
    }
}
