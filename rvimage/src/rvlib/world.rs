use crate::drawme::{Annotation, UpdateImage, UpdateTmpAnno};
use crate::meta_data::MetaData;
use crate::result::trace_ok_err;
use crate::tools::{add_tools_initial_data, get_visible_inactive_names};
use crate::tools_data::annotations::{ClipboardData, InstanceAnnotations};
use crate::tools_data::{
    self, vis_from_lfoption, ExportAsCoco, LabelInfo, ToolSpecifics, ToolsData,
};
use crate::types::ViewImage;
use crate::util::Visibility;
use crate::{image_util, InstanceAnnotate, UpdatePermAnnos, UpdateView, UpdateZoomBox};
use image::DynamicImage;
use rvimage_domain::{BbF, RvError, RvResult, ShapeI};
use std::collections::HashMap;
use std::path::Path;
use std::{fmt::Debug, mem};

pub(super) fn get<'a>(
    world: &'a World,
    actor: &'static str,
    error_msg: &'a str,
) -> RvResult<&'a ToolsData> {
    world
        .data
        .tools_data_map
        .get(actor)
        .ok_or_else(|| RvError::new(error_msg))
}
pub fn get_specific<T>(
    f: impl Fn(&ToolSpecifics) -> RvResult<&T>,
    data: RvResult<&ToolsData>,
) -> Option<&T> {
    trace_ok_err(data.map(|d| &d.specifics).and_then(f))
}
pub(super) fn get_mut<'a>(
    world: &'a mut World,
    actor: &'static str,
    error_msg: &'a str,
) -> RvResult<&'a mut ToolsData> {
    world
        .data
        .tools_data_map
        .get_mut(actor)
        .ok_or_else(|| RvError::new(error_msg))
}
pub fn get_specific_mut<T>(
    f_data_access: impl FnMut(&mut ToolSpecifics) -> RvResult<&mut T>,
    data: RvResult<&mut ToolsData>,
) -> Option<&mut T> {
    trace_ok_err(data.map(|d| &mut d.specifics).and_then(f_data_access))
}

/// Often needed meta data when accessing annotations, see different `AnnoMetaAccessors` structs.
pub trait DataAccess {
    fn get_core_options(world: &World) -> Option<&tools_data::Options>;
    fn get_core_options_mut(world: &mut World) -> Option<&mut tools_data::Options>;
    fn get_track_changes_str(world: &World) -> Option<&'static str>;
    fn get_label_info(world: &World) -> Option<&LabelInfo>;
    fn get_label_info_mut(world: &mut World) -> Option<&mut LabelInfo>;
}

#[macro_export]
macro_rules! tools_data_accessors {
    ($actor_name:expr, $missing_data_msg:expr, $data_module:ident, $data_type:ident, $data_func:ident, $data_func_mut:ident) => {
        #[allow(unused)]
        pub(super) fn get_data(
            world: &World,
        ) -> rvimage_domain::RvResult<&$crate::tools_data::ToolsData> {
            $crate::world::get(world, $actor_name, $missing_data_msg)
        }
        #[allow(unused)]
        pub(super) fn get_specific(world: &World) -> Option<&$data_module::$data_type> {
            $crate::world::get_specific(tools_data::$data_func, get_data(world))
        }
        pub(super) fn get_data_mut(
            world: &mut World,
        ) -> rvimage_domain::RvResult<&mut $crate::tools_data::ToolsData> {
            $crate::world::get_mut(world, $actor_name, $missing_data_msg)
        }
        pub(super) fn get_specific_mut(world: &mut World) -> Option<&mut $data_module::$data_type> {
            $crate::world::get_specific_mut(tools_data::$data_func_mut, get_data_mut(world))
        }
    };
}
#[macro_export]
macro_rules! tools_data_accessors_objects {
    ($actor_name:expr, $missing_data_msg:expr, $data_module:ident, $data_type:ident, $data_func:ident, $data_func_mut:ident) => {
        pub(super) fn get_options(world: &World) -> Option<&$data_module::Options> {
            get_specific(world).map(|d| &d.options)
        }
        pub(super) fn get_options_mut(world: &mut World) -> Option<&mut $data_module::Options> {
            get_specific_mut(world).map(|d| &mut d.options)
        }
        pub(super) fn get_track_changes_str(world: &World) -> Option<&'static str> {
            lazy_static::lazy_static! {
                static ref TRACK_CHANGE_STR: String = $crate::tools::core::make_track_changes_str(ACTOR_NAME);
            };
            let track_changes =
                get_options(world).map(|o| o.core_options.track_changes) == Some(true);
            $crate::util::wrap_if(&TRACK_CHANGE_STR, track_changes)
        }

        pub(super) fn get_label_info(world: &World) -> Option<&LabelInfo> {
            get_specific(world).map(|d| &d.label_info)
        }

        /// when you access annotations, you often also need this metadata
        pub(super) struct DataAccessors;
        impl $crate::world::DataAccess for DataAccessors {
            fn get_core_options(world: &World) -> Option<&$crate::tools_data::Options> {
                get_options(world).map(|o| &o.core_options)
            }
            fn get_core_options_mut(world: &mut World) -> Option<&mut $crate::tools_data::Options> {
                get_options_mut(world).map(|o| &mut o.core_options)
            }
            fn get_track_changes_str(world: &World) -> Option<&'static str> {
                get_track_changes_str(world)
            }
            fn get_label_info(world: &World) -> Option<&LabelInfo> {
                get_label_info(world)
            }
            fn get_label_info_mut(world: &mut World) -> Option<&mut LabelInfo> {
                get_specific_mut(world).map(|d| &mut d.label_info)
            }
        }

        pub(super) fn get_visible(world: &World) -> Visibility {
            let visible = get_options(world).map(|o| o.core_options.visible) == Some(true);
            vis_from_lfoption(get_label_info(world), visible)
        }
        pub(super) fn set_visible(world: &mut World) {
            let options_mut = get_options_mut(world);
            if let Some(options_mut) = options_mut {
                options_mut.core_options.visible = true;
            }
            let vis = get_visible(world);
            world.request_redraw_annotations($actor_name, vis);
        }
    };
}
#[macro_export]
macro_rules! annotations_accessor {
    ($actor_name:expr, $access_func:ident, $error_msg:expr, $annotations_type:ty) => {
        pub(super) fn get_annos_(
            world: &World,
            is_no_anno_fine: bool,
        ) -> Option<&$annotations_type> {
            if let Some(current_file_path) = world.data.meta_data.file_path_relative() {
                let res = world
                    .data
                    .tools_data_map
                    .get($actor_name)
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
    ($actor_name:expr, $access_func:ident, $error_msg:expr, $annotations_type:ty) => {
        pub(super) fn get_annos_mut_(
            world: &mut World,
            is_no_anno_fine: bool,
        ) -> Option<&mut $annotations_type> {
            if let Some(current_file_path) = world.data.meta_data.file_path_relative() {
                let shape_initial = *world.data.shape_initial();
                let res = world
                    .data
                    .tools_data_map
                    .get_mut($actor_name)
                    .and_then(|x| x.specifics.$access_func().ok())
                    .and_then(|d| d.get_annos_mut(&current_file_path, shape_initial));
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
            let is_no_anno_fine = world.data.meta_data.flags.is_file_list_empty == Some(true);
            get_annos_mut_(world, is_no_anno_fine)
        }
    };
}

pub trait InstanceAnnoAccess<T>
where
    T: InstanceAnnotate,
{
    fn get_annos(world: &World) -> Option<&InstanceAnnotations<T>>;
    fn get_annos_mut(world: &mut World) -> Option<&mut InstanceAnnotations<T>>;
    fn get_clipboard(world: &World) -> Option<&ClipboardData<T>>;
    fn set_clipboard(world: &mut World, clipboard: Option<ClipboardData<T>>);
}
#[macro_export]
macro_rules! instance_annotations_accessor {
    ($annotations_type:ty) => {
        pub(super) struct InstanceAnnoAccessors;
        impl $crate::world::InstanceAnnoAccess<$annotations_type> for InstanceAnnoAccessors {
            fn get_annos(world: &World) -> Option<&$crate::tools_data::annotations::InstanceAnnotations<$annotations_type>> {
                get_annos(world)
            }
            fn get_annos_mut(
                world: &mut World,
            ) -> Option<&mut $crate::tools_data::annotations::InstanceAnnotations<$annotations_type>> {
                get_annos_mut(world)
            }
            fn get_clipboard(
                world: &World,
            ) -> Option<&$crate::tools_data::annotations::ClipboardData<$annotations_type>> {
                get_specific(world).and_then(|d| d.clipboard.as_ref())
            }
            fn set_clipboard(
                world: &mut World,
                clipboard: Option<$crate::tools_data::annotations::ClipboardData<$annotations_type>>,
            ) {
                let specific_data = get_specific_mut(world);
                if let Some(d) = specific_data {
                    d.clipboard = clipboard;
                }
            }
        }
    };
}
// tool name -> tool's menu data type
pub type ToolsDataMap = HashMap<String, ToolsData>;

#[derive(Clone, Default, PartialEq)]
pub struct DataRaw {
    im_background: DynamicImage,
    shape_initial: ShapeI,
    pub meta_data: MetaData,
    pub tools_data_map: ToolsDataMap,
}

impl DataRaw {
    pub fn new(
        im_background: DynamicImage,
        meta_data: MetaData,
        tools_data_map: ToolsDataMap,
    ) -> Self {
        let shape_initial = ShapeI::from_im(&im_background);
        DataRaw {
            im_background,
            shape_initial,
            meta_data,
            tools_data_map,
        }
    }

    pub fn im_background(&self) -> &DynamicImage {
        &self.im_background
    }

    pub fn shape_initial(&self) -> &ShapeI {
        &self.shape_initial
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

fn evaluate_visibility(
    visibility: Visibility,
    tool_name: &str,
    data: &DataRaw,
) -> Option<Vec<Annotation>> {
    match (
        visibility,
        &data.meta_data.file_path_relative(),
        data.tools_data_map.get(tool_name),
    ) {
        (Visibility::All, Some(file_path_relative), Some(td)) => {
            td.specifics.to_annotations_view(file_path_relative, None)
        }
        (Visibility::Only(idx), Some(file_path), Some(td)) => {
            td.specifics.to_annotations_view(file_path, Some(idx))
        }
        (Visibility::None, _, _) => Some(vec![]),
        _ => None,
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
                perm_annos: UpdatePermAnnos::No,
                tmp_annos: UpdateTmpAnno::No,
                zoom_box: UpdateZoomBox::Yes(zoom_box),
                image_info: None,
            },
        };
        add_tools_initial_data(world)
    }

    pub fn request_redraw_annotations(&mut self, tool_name: &str, visibility_active: Visibility) {
        let visible_inactive_tools = self
            .data
            .tools_data_map
            .get(tool_name)
            .map(|td| td.visible_inactive_tools.clone());
        let tool_names_inactive = get_visible_inactive_names(tool_name);
        let mut annos_inactive: Option<Vec<Annotation>> = None;
        if let Some(visible_inactive_tools) = visible_inactive_tools {
            for (tool_name_inactive, show) in tool_names_inactive
                .iter()
                .zip(visible_inactive_tools.iter())
            {
                let vli = self.data.tools_data_map.get(*tool_name_inactive).map(|td| {
                    match &td.specifics {
                        tools_data::ToolSpecifics::Bbox(bbox_data) => (
                            bbox_data.options.core_options.visible,
                            bbox_data.label_info(),
                        ),
                        tools_data::ToolSpecifics::Brush(brush_data) => (
                            brush_data.options.core_options.visible,
                            brush_data.label_info(),
                        ),
                        _ => {
                            panic!("tool {tool_name_inactive} does not redraw annotations ");
                        }
                    }
                });
                let visibility_inactive = if let Some((visible, label_info)) = vli {
                    vis_from_lfoption(Some(label_info), visible)
                } else {
                    Visibility::All
                };
                if show && visibility_active != Visibility::None {
                    if let Some(annos) = &mut annos_inactive {
                        let annos_inner = evaluate_visibility(
                            visibility_inactive,
                            tool_name_inactive,
                            &self.data,
                        );
                        if let Some(annos_inner) = annos_inner {
                            annos.extend(annos_inner);
                        }
                    } else {
                        annos_inactive = evaluate_visibility(
                            visibility_inactive,
                            tool_name_inactive,
                            &self.data,
                        );
                    }
                }
            }
        }
        let annos_active = evaluate_visibility(visibility_active, tool_name, &self.data);
        if let Some(annos_active) = annos_active {
            if let Some(annos_inactive) = annos_inactive {
                let mut annos = annos_active;
                annos.extend(annos_inactive);
                self.update_view.perm_annos = UpdatePermAnnos::Yes(annos);
            } else {
                self.update_view.perm_annos = UpdatePermAnnos::Yes(annos_active);
            }
        } else if let Some(annos_inactive) = annos_inactive {
            self.update_view.perm_annos = UpdatePermAnnos::Yes(annos_inactive);
        }
    }

    pub fn request_redraw_tmp_anno(&mut self, anno: Annotation) {
        self.update_view.tmp_annos = UpdateTmpAnno::Yes(anno);
    }

    pub fn stop_tmp_anno(&mut self) {
        self.update_view.tmp_annos = UpdateTmpAnno::No;
    }

    pub fn request_redraw_image(&mut self) {
        if self.data.meta_data.file_path_relative().is_some() {
            self.update_view.image = UpdateImage::Yes(self.data.bg_to_uncropped_view())
        }
    }

    /// real image in contrast to the loading image
    pub fn from_real_im(
        im: DynamicImage,
        tools_data: ToolsDataMap,
        file_path: Option<String>,
        prj_path: &Path,
        file_selected_idx: Option<usize>,
    ) -> Self {
        let meta_data = match (file_path, file_selected_idx) {
            (Some(fp), Some(fsidx)) => MetaData::from_filepath(fp, fsidx, prj_path),
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
        write!(f, "\nims_raw {:?}", &self.data)
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
