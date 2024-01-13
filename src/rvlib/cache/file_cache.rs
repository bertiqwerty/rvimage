use std::{collections::HashMap, fmt::Debug, fs, marker::PhantomData, path::Path};

use crate::{
    cache::core::Cache,
    file_util, image_util,
    result::{to_rv, RvError, RvResult},
    rverr,
    threadpool::ThreadPoolQueued,
    types::{AsyncResultImage, ImageInfoPair, ResultImage},
};

use serde::{Deserialize, Serialize};

use super::ReadImageToCache;

fn copy<F>(path_or_url: &str, reader: F, target: &str) -> RvResult<()>
where
    F: Fn(&str) -> ResultImage,
{
    let im = reader(path_or_url)?;
    im.save(target)
        .map_err(|e| rverr!("could not save image to {:?}. {}", target, e.to_string()))?;
    Ok(())
}

fn preload<'a, I, RTC, A>(
    files: I,
    tp: &mut ThreadPoolQueued<RvResult<String>>,
    reader: &RTC,
    tmpdir: &str,
) -> RvResult<HashMap<String, ThreadResult>>
where
    I: Iterator<Item = (usize, &'a str)>,
    RTC: ReadImageToCache<A> + Clone + Send + 'static,
{
    let delay_ms = 10;
    fs::create_dir_all(Path::new(tmpdir)).map_err(to_rv)?;
    files
        .map(|(prio, file)| {
            let dst_file = file_util::filename_in_tmpdir(file, tmpdir)?;
            let file_for_thread = file.to_string();
            let reader_for_thread = reader.clone();
            let job = Box::new(move || {
                match copy(&file_for_thread, |p| reader_for_thread.read(p), &dst_file) {
                    Ok(_) => Ok(dst_file),
                    Err(e) => Err(e),
                }
            });
            Ok((
                file.to_string(),
                ThreadResult::Running(tp.apply(job, prio, delay_ms)?),
            ))
        })
        .collect::<RvResult<HashMap<_, _>>>()
}

#[derive(Debug)]
struct LocalImagePathInfoPair {
    path: String,
    info: String,
}

#[derive(Debug)]
enum ThreadResult {
    Running(u128),
    Ok(LocalImagePathInfoPair),
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct FileCacheCfgArgs {
    pub n_prev_images: usize,
    pub n_next_images: usize,
    pub n_threads: usize,
}
#[derive(Clone)]
pub struct FileCacheArgs<RA> {
    pub cfg_args: FileCacheCfgArgs,
    pub reader_args: RA,
    pub tmpdir: String,
}

pub struct FileCache<RTC, RA>
where
    RTC: ReadImageToCache<RA>,
{
    cached_paths: HashMap<String, ThreadResult>,
    n_prev_images: usize,
    n_next_images: usize,
    tpq: ThreadPoolQueued<RvResult<String>>,
    tmpdir: String,
    reader: RTC,
    reader_args_phantom: PhantomData<RA>,
}
impl<RTC, RA> Cache<FileCacheArgs<RA>> for FileCache<RTC, RA>
where
    RTC: ReadImageToCache<RA> + Send + Clone + 'static,
{
    fn ls(&self, folder_path: &str) -> RvResult<Vec<String>> {
        self.reader.ls(folder_path)
    }
    fn load_from_cache(
        &mut self,
        selected_file_idx: usize,
        files: &[&str],
        reload: bool,
    ) -> AsyncResultImage {
        if files.is_empty() {
            return Err(RvError::new("no files to read from"));
        }
        if reload {
            self.cached_paths.clear();
            let tmp_path = Path::new(&self.tmpdir);
            if tmp_path.exists() {
                fs::remove_dir_all(tmp_path).map_err(to_rv)?;
            }
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
        let indices_to_iterate = start_idx..end_idx;
        let n_max_possible_files = self.n_prev_images + self.n_next_images + 1;
        let prio_file_pairs = indices_to_iterate.map(|idx| {
            (
                n_max_possible_files
                    - (selected_file_idx as i32 - idx as i32).unsigned_abs() as usize,
                &files[idx],
            )
        });
        // update priorities of in cache files
        let files_in_cache = prio_file_pairs
            .clone()
            .filter(|(_, file)| self.cached_paths.contains_key(**file));
        for (prio, file) in files_in_cache {
            if let ThreadResult::Running(job_id) = self.cached_paths[*file] {
                self.tpq.update_prio(job_id, Some(prio))?;
            }
        }
        // trigger caching of not in cache files
        let files_not_in_cache = prio_file_pairs
            .filter(|(_, file)| !self.cached_paths.contains_key(**file))
            .map(|(i, file)| (i, *file));
        let cache = preload(
            files_not_in_cache,
            &mut self.tpq,
            &self.reader,
            &self.tmpdir,
        )?;
        // update cache
        for elt in cache.into_iter() {
            let (file, th_res) = elt;
            self.cached_paths.insert(file, th_res);
        }
        let selected_file = files[selected_file_idx];
        let selected_file_state = &self.cached_paths[selected_file];
        match selected_file_state {
            ThreadResult::Ok(path_info_pair) => {
                let LocalImagePathInfoPair { path, info } = path_info_pair;
                image_util::read_image(path).map(|im| {
                    Some(ImageInfoPair {
                        im,
                        info: info.clone(),
                    })
                })
            }
            ThreadResult::Running(job_id) => {
                let path_in_cache = self.tpq.result(*job_id);
                match path_in_cache {
                    Some(pic) => {
                        let pic = pic?;
                        let res = image_util::read_image(&pic);
                        let info = self
                            .reader
                            .file_info(selected_file)
                            .unwrap_or_else(|_| file_util::local_file_info(&pic));
                        *self.cached_paths.get_mut(selected_file).unwrap() =
                            ThreadResult::Ok(LocalImagePathInfoPair {
                                path: pic,
                                info: info.clone(),
                            });
                        res.map(|im| Some(ImageInfoPair { im, info }))
                    }
                    None => Ok(None),
                }
            }
        }
    }
    fn new(args: FileCacheArgs<RA>) -> RvResult<Self> {
        let FileCacheCfgArgs {
            n_prev_images,
            n_next_images,
            n_threads,
        } = args.cfg_args;
        let tp = ThreadPoolQueued::new(n_threads);
        Ok(Self {
            cached_paths: HashMap::new(),
            n_prev_images,
            n_next_images,
            tpq: tp,
            tmpdir: args.tmpdir,
            reader: RTC::new(args.reader_args)?,
            reader_args_phantom: PhantomData {},
        })
    }
}

#[cfg(test)]
use {
    crate::{cfg, defer_folder_removal},
    image::DynamicImage,
    image::{ImageBuffer, Rgb},
    std::{thread, time::Duration},
};

#[test]
fn test_file_cache() -> RvResult<()> {
    let cfg = cfg::read_cfg()?;
    let tmpdir_path = Path::new(cfg.tmpdir()?);
    fs::create_dir_all(tmpdir_path).map_err(to_rv)?;
    defer_folder_removal!(&tmpdir_path);
    let test = |files: &[&str], selected: usize| -> RvResult<()> {
        #[derive(Clone)]
        struct DummyRead;
        impl ReadImageToCache<()> for DummyRead {
            fn new(_: ()) -> RvResult<Self> {
                Ok(Self {})
            }
            fn ls(&self, _folder_path: &str) -> RvResult<Vec<String>> {
                Ok(vec![])
            }
            fn read(&self, _: &str) -> RvResult<DynamicImage> {
                let dummy_image =
                    DynamicImage::ImageRgb8(ImageBuffer::<Rgb<u8>, Vec<u8>>::new(20, 20));
                Ok(dummy_image)
            }
            fn file_info(&self, _: &str) -> RvResult<String> {
                Ok("".to_string())
            }
        }

        let file_cache_args = FileCacheArgs {
            cfg_args: FileCacheCfgArgs {
                n_prev_images: 2,
                n_next_images: 8,
                n_threads: 2,
            },
            reader_args: (),
            tmpdir: cfg.tmpdir()?.to_string(),
        };
        let mut cache = FileCache::<DummyRead, ()>::new(file_cache_args)?;
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
        let reload = false;
        cache.load_from_cache(selected, files, reload)?;
        let n_millis = (max_i - min_i) * 100;
        println!("waiting {} millis", n_millis);
        thread::sleep(Duration::from_millis(n_millis as u64));

        for (_, file) in files
            .iter()
            .enumerate()
            .filter(|(i, _)| min_i <= *i && *i < max_i)
        {
            println!(
                "filename in tmpdir {:?}",
                Path::new(file_util::filename_in_tmpdir(file, cfg.tmpdir()?)?.as_str())
            );
            assert!(
                Path::new(file_util::filename_in_tmpdir(file, cfg.tmpdir()?)?.as_str()).exists()
            );
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
        assert!(
            Path::new(file_util::filename_in_tmpdir(f.as_str(), cfg.tmpdir()?)?.as_str()).exists()
        );
    }
    Ok(())
}
