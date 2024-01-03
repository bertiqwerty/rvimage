use crate::domain::{BbF, ShapeI};
use crate::drawme::{Annotation, UpdateImage};
use crate::file_util::MetaData;
use crate::tools::add_tools_initial_data;
use crate::tools_data::ToolsData;
use crate::types::ViewImage;
use crate::util::Visibility;
use crate::{image_util, UpdateAnnos, UpdateView, UpdateZoomBox};
use image::DynamicImage;
use std::collections::HashMap;
use std::{fmt::Debug, mem};

#[macro_export]
macro_rules! annotations_accessor {
    ($actor:expr, $access_func:ident, $error_msg:expr, $annotations_type:ty) => {
        pub(super) fn get_annos_(
            world: &World,
            is_no_anno_fine: bool,
        ) -> Option<&$annotations_type> {
            if let Some(current_file_path) = world.data.meta_data.file_path.as_ref() {
                let res = world
                    .data
                    .tools_data_map
                    .get($actor)
                    .and_then(|x| x.specifics.$access_func().ok())
                    .and_then(|d| d.get_annos(&current_file_path));
                if res.is_none() && !is_no_anno_fine {
                    tracing::error!("{}", $error_msg);
                }
                res
            } else {
                None
            }
        }
        #[allow(unused)]
        pub(super) fn get_annos(world: &World) -> Option<&$annotations_type> {
            get_annos_(world, false)
        }
        #[allow(unused)]
        pub(super) fn get_annos_if_some(world: &World) -> Option<&$annotations_type> {
            get_annos_(world, true)
        }
    };
}
#[macro_export]
macro_rules! annotations_accessor_mut {
    ($actor:expr, $access_func:ident, $error_msg:expr, $annotations_type:ty) => {
        pub(super) fn get_annos_mut_(
            world: &mut World,
            is_no_anno_fine: bool,
        ) -> Option<&mut $annotations_type> {
            if let Some(current_file_path) = world.data.meta_data.file_path.as_ref() {
                let shape = world.data.shape();
                let res = world
                    .data
                    .tools_data_map
                    .get_mut($actor)
                    .and_then(|x| x.specifics.$access_func().ok())
                    .and_then(|d| d.get_annos_mut(&current_file_path, shape));
                if res.is_none() {
                    tracing::error!("{}", $error_msg);
                }
                res
            } else {
                if !is_no_anno_fine {
                    tracing::error!("could not find filepath in meta data")
                };
                None
            }
        }
        pub(super) fn get_annos_mut(world: &mut World) -> Option<&mut $annotations_type> {
            let is_no_anno_fine = world.data.meta_data.is_file_list_empty == Some(true);
            get_annos_mut_(world, is_no_anno_fine)
        }
    };
}

// tool name -> tool's menu data type
pub type ToolsDataMap = HashMap<String, ToolsData>;

#[derive(Clone, Default, PartialEq)]
pub struct DataRaw {
    im_background: DynamicImage,
    pub meta_data: MetaData,
    pub tools_data_map: ToolsDataMap,
}

impl DataRaw {
    pub fn current_file_path(&self) -> &Option<String> {
        &self.meta_data.file_path
    }
    pub fn new(
        im_background: DynamicImage,
        meta_data: MetaData,
        tools_data_map: ToolsDataMap,
    ) -> Self {
        DataRaw {
            im_background,
            meta_data,
            tools_data_map,
        }
    }

    pub fn im_background(&self) -> &DynamicImage {
        &self.im_background
    }

    pub fn apply<FI>(&mut self, mut f_i: FI)
    where
        FI: FnMut(DynamicImage) -> DynamicImage,
    {
        self.im_background = f_i(mem::take(&mut self.im_background));
    }

    pub fn shape(&self) -> ShapeI {
        ShapeI::from_im(&self.im_background)
    }

    pub fn bg_to_uncropped_view(&self) -> ViewImage {
        image_util::orig_to_0_255(&self.im_background, &None)
    }
}

impl Debug for DataRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nshape {:?}\ntools data {:?}",
            self.shape(),
            self.tools_data_map,
        )
    }
}

/// Everything we need to draw
#[derive(Clone, Default)]
pub struct World {
    pub update_view: UpdateView,
    pub data: DataRaw,
    // transforms coordinates from view to raw image
    zoom_box: Option<BbF>,
}

impl World {
    pub fn new(ims_raw: DataRaw, zoom_box: Option<BbF>) -> Self {
        let im = ims_raw.bg_to_uncropped_view();
        let world = Self {
            data: ims_raw,
            zoom_box,
            update_view: UpdateView {
                image: UpdateImage::Yes(im),
                annos: UpdateAnnos::No,
                zoom_box: UpdateZoomBox::Yes(zoom_box),
                image_info: None,
            },
        };
        add_tools_initial_data(world)
    }

    pub fn request_redraw_annotations(&mut self, tool_name: &str, visibility: Visibility) {
        match (
            visibility,
            &self.data.meta_data.file_path,
            self.data.tools_data_map.get(tool_name),
        ) {
            (Visibility::All, Some(file_path), Some(td)) => {
                self.update_view.annos = td.specifics.to_annotations_view(file_path, None);
            }

            (Visibility::Only(idx), Some(file_path), Some(td)) => {
                self.update_view.annos = td.specifics.to_annotations_view(file_path, Some(idx));
            }

            (Visibility::None, _, _) => {
                self.update_view.annos = UpdateAnnos::clear();
            }
            _ => (),
        }
    }

    pub fn request_redraw_tmp_anno(&mut self, anno: Annotation) {
        self.update_view.annos = match &mut self.update_view.annos {
            UpdateAnnos::No => UpdateAnnos::Yes((vec![], Some(anno))),
            UpdateAnnos::Yes((perma_annos, _)) => {
                UpdateAnnos::Yes((std::mem::take(perma_annos), Some(anno)))
            }
        }
    }

    pub fn stop_tmp_anno(&mut self) {
        self.update_view.annos = match &mut self.update_view.annos {
            // hmm... this might override other permanent annos that were not updated recently
            UpdateAnnos::No => UpdateAnnos::clear(),
            UpdateAnnos::Yes((perma_annos, _)) => {
                UpdateAnnos::Yes((std::mem::take(perma_annos), None))
            }
        }
    }

    pub fn request_redraw_image(&mut self) {
        if self.data.meta_data.file_path.is_some() {
            self.update_view.image = UpdateImage::Yes(self.data.bg_to_uncropped_view())
        }
    }

    /// real image in contrast to the loading image
    pub fn from_real_im(
        im: DynamicImage,
        tools_data: ToolsDataMap,
        file_path: Option<String>,
        file_selected_idx: Option<usize>,
    ) -> Self {
        let meta_data = match (file_path, file_selected_idx) {
            (Some(fp), Some(fsidx)) => MetaData::from_filepath(fp, fsidx),
            _ => MetaData::default(),
        };
        Self::new(DataRaw::new(im, meta_data, tools_data), None)
    }

    pub fn shape_orig(&self) -> ShapeI {
        self.data.shape()
    }

    pub fn set_zoom_box(&mut self, zoom_box: Option<BbF>) {
        let mut set_zb = || {
            self.zoom_box = zoom_box;
            self.update_view = UpdateView::from_zoombox(zoom_box);
        };
        if let Some(zb) = zoom_box {
            if zb.h > 1.0 && zb.w > 1.0 {
                set_zb();
            }
        } else {
            set_zb();
        }
    }

    pub fn zoom_box(&self) -> &Option<BbF> {
        &self.zoom_box
    }
}
impl Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\nims_raw {:?}", &self.data,)
    }
}

#[cfg(test)]
fn rgba_at(i: usize, im: &ViewImage) -> [u8; 4] {
    let x = (i % im.width() as usize) as u32;
    let y = (i / im.width() as usize) as u32;
    let rgb = im.get_pixel(x, y).0;
    let rgb_changed = rgb;
    [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff]
}
#[cfg(test)]
use image::Rgb;

#[test]
fn test_rgba() {
    let mut im_test = ViewImage::new(64, 64);
    im_test.put_pixel(0, 0, Rgb([23, 23, 23]));
    assert_eq!(rgba_at(0, &im_test), [23, 23, 23, 255]);
    im_test.put_pixel(0, 1, Rgb([23, 23, 23]));
    assert_eq!(rgba_at(64, &im_test), [23, 23, 23, 255]);
    im_test.put_pixel(7, 11, Rgb([23, 23, 23]));
    assert_eq!(rgba_at(11 * 64 + 7, &im_test), [23, 23, 23, 255]);
}
