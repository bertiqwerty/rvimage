use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use image::{ImageBuffer, Rgb};

use crate::util;

type ResultImage = io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>>;

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

pub fn filename_in_tmpdir(path: &str) -> io::Result<PathBuf> {
    let path = PathBuf::from_str(path).unwrap();
    let fname = util::osstr_to_str(path.file_name())?;
    Ok(std::env::temp_dir().join(fname))
}

fn copy<F>(path_or_url: &str, reader: F, target: &Path) -> io::Result<()>
where
    F: Fn(&str) -> ResultImage,
{
    let im = reader(path_or_url)?;
    im.save(&target).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("could not save image to {:?}. {}", target, e.to_string()),
        )
    })?;
    Ok(())
}

struct PathPair {
    origin_path_or_url: String,
    path_in_cache: PathBuf,
}
type CopyState = io::Result<PathPair>;
pub trait ReaderType: Fn(&str) -> ResultImage + Send + Sync + Clone + 'static {}
impl<T: Fn(&str) -> ResultImage + Send + Sync + Clone + 'static> ReaderType for T {}

fn preload<F: ReaderType>(
    files: &[String],
    started_copies: &HashSet<String>,
    tx: &Sender<CopyState>,
    reader: &F,
) -> io::Result<()> {
    for file in files.iter().filter(|f| !started_copies.contains(*f)) {
        let tmp_file = filename_in_tmpdir(&file)?;
        let file = file.clone();
        let tx = tx.clone();
        let reader = reader.clone();
        thread::spawn(move || match copy(&file, reader, &tmp_file) {
            Ok(_) => tx.send(CopyState::Ok(PathPair {
                origin_path_or_url: file.clone(),
                path_in_cache: tmp_file,
            })),
            Err(e) => tx.send(CopyState::Err(e)),
        });
    }
    Ok(())
}

fn update_cache(
    rx: &mut Receiver<CopyState>,
    cached_paths: &mut HashMap<String, PathBuf>,
) -> io::Result<()> {
    for received in rx.try_iter() {
        match received {
            CopyState::Ok(pp) => {
                cached_paths.insert(pp.origin_path_or_url, pp.path_in_cache);
            }
            CopyState::Err(e) => {
                return Err(e);
            }
        }
    }
    Ok(())
}
pub struct FileCache<F = DefaultReader>
where
    F: ReaderType,
{
    reader: F,
    cached_paths: HashMap<String, PathBuf>,
    started_copies: HashSet<String>,
    half_n_images: usize,
    tx: Sender<CopyState>,
    rx: Receiver<CopyState>,
}
impl<F> Preload<F> for FileCache<F>
where
    F: Fn(&str) -> ResultImage + Send + Sync + Clone + 'static,
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
        for ftp in files_to_preload {
            self.started_copies.insert(ftp.clone());
        }
        preload(files_to_preload, &self.started_copies, &self.tx, &self.reader)?;
        update_cache(&mut self.rx, &mut self.cached_paths)?;

        (self.reader)(&files[selected_file_idx])
    }
    fn new(reader: F) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            reader,
            cached_paths: HashMap::new(),
            started_copies: HashSet::new(),
            half_n_images: 5,
            tx,
            rx,
        }
    }
}
