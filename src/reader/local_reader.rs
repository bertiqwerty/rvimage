use walkdir::WalkDir;

use crate::{
    cache::ReadImageToCache, 
    result::{to_rv, RvError, RvResult},
    util, types::ResultImage,
};

use super::core::{path_to_str, CloneDummy, PickFolder, Picked, SUPPORTED_EXTENSIONS};

fn read_image_paths(path: &str) -> RvResult<Vec<String>> {
    WalkDir::new(path)
        .into_iter()
        .map(|p| p.map_err(to_rv))
        .filter(|p| match p {
            Err(_) => true,
            Ok(p_) => match p_.path().extension() {
                Some(ext) => SUPPORTED_EXTENSIONS
                    .iter()
                    .find(|sup_ext| Some(&sup_ext[1..]) == ext.to_str())
                    .is_some(),
                None => false,
            },
        })
        .map(|p| Ok(path_to_str(p?.path())?.to_string()))
        .collect::<RvResult<Vec<String>>>()
}
pub struct FileDialogPicker;
impl PickFolder for FileDialogPicker {
    fn pick() -> RvResult<Picked> {
        let sf = rfd::FileDialog::new()
            .pick_folder()
            .ok_or_else(|| RvError::new("Could not pick folder."))?;
        let path_as_string: String = sf
            .to_str()
            .ok_or_else(|| RvError::new("could not transfer path to unicode string"))?
            .to_string();
        let image_paths = read_image_paths(&path_as_string)?;
        Ok(Picked {
            folder_path: path_as_string,
            file_paths: image_paths,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ReadImageFromPath;
impl ReadImageToCache<CloneDummy> for ReadImageFromPath {
    fn new(_: CloneDummy) -> RvResult<Self> {
        Ok(Self {})
    }
    fn read(&self, path: &str) -> ResultImage {
        util::read_image(path)
    }
}

#[cfg(test)]
use {
    crate::{
        cache::NoCache,
        format_rverr,
        reader::core::{LoadImageForGui, Loader},
        types::ViewImage,
    },
    std::{env, fs},
};
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
#[test]
fn test_folder_reader() -> RvResult<()> {
    let tmp_dir = env::temp_dir().join(TMP_SUBFOLDER);
    match fs::remove_dir_all(&tmp_dir) {
        Ok(_) => (),
        Err(_) => (),
    }
    fs::create_dir(&tmp_dir).map_err(to_rv)?;
    for i in 0..10 {
        let im = ViewImage::new(10, 10);
        let out_path = tmp_dir.join(format!("tmpfile_{}.png", i));
        im.save(out_path).unwrap();
    }
    let mut reader =
        Loader::<NoCache<ReadImageFromPath, _>, TmpFolderPicker, _>::new(CloneDummy {}, 0)?;
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
