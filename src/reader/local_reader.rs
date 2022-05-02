use crate::result::{RvError, RvResult};

use super::core::{read_image_paths, PickFolder, Picked};

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
        let image_paths =  read_image_paths(&path_as_string)?;
        Ok(Picked{folder_path: path_as_string, file_paths: image_paths})
    }
}
