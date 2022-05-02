use std::{collections::HashMap, fmt::Debug, fs, marker::PhantomData, path::Path};

use crate::{
    cache::core::Cache,
    cfg, format_rverr,
    result::{to_rv, AsyncResultImage, ResultImage, RvError, RvResult},
    threadpool::ThreadPool,
    util::filename_in_tmpdir,
};

use serde::Deserialize;

use super::ReadImageToCache;

fn copy<F>(path_or_url: &str, reader: F, target: &str) -> RvResult<()>
where
    F: Fn(&str) -> ResultImage,
{
    let im = reader(path_or_url).map_err(to_rv)?;
    im.save(&target)
        .map_err(|e| format_rverr!("could not save image to {:?}. {}", target, e.to_string()))?;
    Ok(())
}

fn preload<IR: ReadImageToCache>(
    files: &[String],
    tp: &mut ThreadPool<RvResult<String>>,
    cache: &HashMap<String, ThreadResult>,
    tmpdir: &str,
) -> RvResult<HashMap<String, ThreadResult>> {
    fs::create_dir_all(Path::new(tmpdir)).map_err(to_rv)?;
    files
        .iter()
        .filter(|file| !cache.contains_key(*file))
        .map(|file| {
            let dst_file = filename_in_tmpdir(file, tmpdir)?;
            let file_for_thread = file.clone();
            let job = Box::new(move || match copy(&file_for_thread, IR::read, &dst_file) {
                Ok(_) => Ok(dst_file),
                Err(e) => Err(e),
            });
            Ok((file.clone(), ThreadResult::Running(tp.apply(job)?)))
        })
        .collect::<RvResult<HashMap<_, _>>>()
}

#[derive(Debug)]
enum ThreadResult {
    Running(usize),
    Ok(String),
}

#[derive(Deserialize, Debug)]
pub struct FileCacheArgs {
    n_prev_images: usize,
    n_next_images: usize,
    n_threads: usize,
}

pub struct FileCache<RIC>
where
    RIC: ReadImageToCache,
{
    cached_paths: HashMap<String, ThreadResult>,
    n_prev_images: usize,
    n_next_images: usize,
    tp: ThreadPool<RvResult<String>>,
    reader_phantom: PhantomData<RIC>,
}
impl<RIC> Cache<FileCacheArgs> for FileCache<RIC>
where
    RIC: ReadImageToCache,
{
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> AsyncResultImage {
        let cfg = cfg::get_cfg()?;
        if files.is_empty() {
            return Err(RvError::new("no files to read from"));
        }
        let start_idx = if selected_file_idx <= self.n_prev_images {
            0
        } else {
            selected_file_idx - self.n_prev_images
        };
        let end_idx = if files.len() <= selected_file_idx + self.n_next_images {
            files.len()
        } else {
            selected_file_idx + self.n_next_images + 1
        };
        let files_to_preload = &files[start_idx..end_idx];
        let cache = preload::<RIC>(
            files_to_preload,
            &mut self.tp,
            &self.cached_paths,
            cfg.tmpdir()?,
        )?;
        // update cache
        for elt in cache.into_iter() {
            let (file, th_res) = elt;
            self.cached_paths.insert(file, th_res);
        }
        let selected_file = &files[selected_file_idx];
        let selected_file_state = &self.cached_paths[selected_file];
        match selected_file_state {
            ThreadResult::Ok(path_in_cache) => RIC::read(path_in_cache).map(Some),
            ThreadResult::Running(job_id) => {
                let path_in_cache = self.tp.result(*job_id);
                match path_in_cache {
                    Some(pic) => {
                        let pic = pic?;
                        let res = RIC::read(&pic);
                        *self.cached_paths.get_mut(selected_file).unwrap() = ThreadResult::Ok(pic);
                        res.map(Some)
                    }
                    None => Ok(None),
                }
            }
        }
    }
    fn new(args: FileCacheArgs) -> Self {
        let FileCacheArgs {
            n_prev_images,
            n_next_images,
            n_threads,
        } = args;
        let tp = ThreadPool::new(n_threads);
        Self {
            cached_paths: HashMap::new(),
            n_prev_images,
            n_next_images,
            tp,
            reader_phantom: PhantomData {},
        }
    }
}

#[cfg(test)]
use {
    crate::ImageType,
    image::{ImageBuffer, Rgb},
    std::{thread, time::Duration},
};

#[test]
fn test_file_cache() -> RvResult<()> {
    let cfg = cfg::get_cfg()?;
    let tmpdir_path = Path::new(cfg.tmpdir()?);
    fs::create_dir_all(tmpdir_path).map_err(to_rv)?;
    let test = |files: &[&str], selected: usize| -> RvResult<()> {
        struct DummyRead;
        impl ReadImageToCache for DummyRead {
            fn read(_: &str) -> RvResult<ImageType> {
                let dummy_image = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(20, 20);
                Ok(dummy_image)
            }
        }
        let file_cache_args = FileCacheArgs {
            n_prev_images: 2,
            n_next_images: 8,
            n_threads: 2,
        };
        let mut cache = FileCache::<DummyRead>::new(file_cache_args);
        let min_i = if selected > cache.n_prev_images {
            selected - cache.n_prev_images
        } else {
            0
        };
        let max_i = if selected + cache.n_next_images > files.len() {
            files.len()
        } else {
            selected + cache.n_next_images
        };
        let files = files.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        cache.read_image(selected, &files)?;
        let n_millis = (max_i - min_i) * 100;
        println!("waiting {} millis", n_millis);
        thread::sleep(Duration::from_millis(n_millis as u64));

        for (_, file) in files
            .iter()
            .enumerate()
            .filter(|(i, _)| min_i <= *i && *i < max_i)
        {
            let f = file.as_str();
            println!(
                "filename in tmpdir {:?}",
                Path::new(filename_in_tmpdir(f, cfg.tmpdir()?)?.as_str())
            );
            assert!(Path::new(filename_in_tmpdir(f, cfg.tmpdir()?)?.as_str()).exists());
        }
        Ok(())
    };
    assert!(test(&[], 0).is_err());
    test(&["1.png", "2.png", "3.png", "4.png"], 0)?;
    test(&["1.png", "2.png", "3.png", "4.png"], 1)?;
    test(&["1.png", "2.png", "3.png", "4.png"], 2)?;
    test(&["1.png", "2.png", "3.png", "4.png"], 3)?;
    let files = (0..50).map(|i| format!("{}.png", i)).collect::<Vec<_>>();
    let files_str = files.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    test(&files_str, 16)?;
    test(&files_str, 36)?;
    for i in (14..25).chain(34..45) {
        let f = format!("{}.png", i);
        assert!(Path::new(filename_in_tmpdir(f.as_str(), cfg.tmpdir()?)?.as_str()).exists());
    }
    fs::remove_dir_all(tmpdir_path).map_err(to_rv)?;
    Ok(())
}
