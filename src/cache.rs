use std::{collections::HashMap, fs, path::PathBuf, str::FromStr};

use image::{ImageBuffer, Rgb};

use crate::{
    format_rverr,
    result::{to_rv, RvError, RvResult},
    threadpool::ThreadPool,
    util,
};

type ResultImage = RvResult<ImageBuffer<Rgb<u8>, Vec<u8>>>;

type DefaultReader = fn(&str) -> ResultImage;

pub trait ReaderType: Fn(&str) -> ResultImage + Send + Sync + Clone + 'static {}
impl<T: Fn(&str) -> ResultImage + Send + Sync + Clone + 'static> ReaderType for T {}

pub trait Preload<F = DefaultReader>
where
    F: ReaderType,
{
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage;
    fn new(reader: F) -> Self;
}

pub struct NoCache<F = DefaultReader>
where
    F: ReaderType,
{
    reader: F,
}

impl<F> Preload<F> for NoCache<F>
where
    F: ReaderType,
{
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage {
        (self.reader)(&files[selected_file_idx])
    }
    fn new(reader: F) -> Self {
        Self { reader }
    }
}

fn filename_in_tmpdir(path: &str) -> RvResult<String> {
    let path = PathBuf::from_str(path).unwrap();
    let fname = util::osstr_to_str(path.file_name()).map_err(to_rv)?;
    let tmpfolder = std::env::temp_dir().join("rimview");
    fs::create_dir_all(tmpfolder.clone()).map_err(to_rv)?;
    tmpfolder
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
    n_prev_images: usize,
    n_next_images: usize,
    tp: ThreadPool<RvResult<String>>,
}
impl<F> Preload<F> for FileCache<F>
where
    F: ReaderType,
{
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage {
        if files.len() == 0 {
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
        let n_prev_images = 2;
        let n_next_images = 8;
        let n_threads = 5;
        let tp = ThreadPool::new(n_threads);
        Self {
            reader,
            cached_paths: HashMap::new(),
            n_prev_images,
            n_next_images,
            tp,
        }
    }
}

#[cfg(test)]
use std::{path::Path, thread, time::Duration};
#[test]
fn test_file_cache() -> RvResult<()> {
    fn test(files: &[&str], selected: usize) -> RvResult<()> {
        fn dummy_read(_: &str) -> RvResult<ImageBuffer<Rgb<u8>, Vec<u8>>> {
            let dummy_image = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(20, 20);
            Ok(dummy_image)
        }
        let mut cache = FileCache::new(dummy_read);
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
        let n_secs = (max_i - min_i) / 4 + 1;
        println!("waiting {} secs", n_secs);
        thread::sleep(Duration::from_secs(n_secs as u64));
        
        for (_, file) in files
            .iter()
            .enumerate()
            .filter(|(i, _)| min_i <= *i && *i < max_i)
        {
            let f = file.as_str();
            println!(
                "filename in tmpdir {:?}",
                Path::new(filename_in_tmpdir(f)?.as_str())
            );
            assert!(Path::new(filename_in_tmpdir(f)?.as_str()).exists());
        }
        Ok(())
    }
    assert!(test(&[], 0).is_err());
    test(&["1.png", "2.png", "3.png", "4.png"], 0)?;
    test(&["1.png", "2.png", "3.png", "4.png"], 1)?;
    test(&["1.png", "2.png", "3.png", "4.png"], 2)?;
    test(&["1.png", "2.png", "3.png", "4.png"], 3)?;
    let files = (0..50).map(|i| format!("{}.png", i)).collect::<Vec<_>>();
    let files_str = files.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    test(&files_str, 16)?;
    // test(&files_str, 36)?;
    // for i in (14..27).chain(34..47) {
    //     let f = format!("{}.png", i);
    //     assert!(Path::new(filename_in_tmpdir(f.as_str())?.as_str()).exists());
    // }

    Ok(())
}
