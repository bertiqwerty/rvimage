use crate::drawme::{Annotation, UpdateImage, UpdateTmpAnno};
use crate::meta_data::MetaData;
use crate::result::trace_ok_err;
use crate::tools::{add_tools_initial_data, get_visible_inactive_names};
use crate::tools_data::annotations::{ClipboardData, InstanceAnnotations};
use crate::tools_data::predictive_labeling::PredictiveLabelingData;
use crate::tools_data::{
    self, vis_from_lfoption, AccessInstanceData, LabelInfo, ToolSpecifics, ToolsData, ToolsDataMap,
};
use crate::types::ViewImage;
use crate::util::Visibility;
use crate::{image_util, InstanceAnnotate, UpdatePermAnnos, UpdateView};
use image::DynamicImage;
use rvimage_domain::{BbF, RvError, RvResult, ShapeF, ShapeI};
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

/// Often needed meta data when accessing annotations
pub trait MetaDataAccess {
    fn get_core_options(world: &World) -> Option<&tools_data::Options>;
    fn get_core_options_mut(world: &mut World) -> Option<&mut tools_data::Options>;
    fn get_track_changes_str(world: &World) -> Option<&'static str>;
    fn get_label_info(world: &World) -> Option<&LabelInfo>;
    fn get_label_info_mut(world: &mut World) -> Option<&mut LabelInfo>;
    fn get_predictive_labeling_data(world: &World) -> Option<&PredictiveLabelingData>;
    fn get_predictive_labeling_data_mut(world: &mut World) -> Option<&mut PredictiveLabelingData>;
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
        pub fn get_specific(
            world: &World,
        ) -> Option<&$crate::tools_data::$data_module::$data_type> {
            $crate::world::get_specific($crate::tools_data::$data_func, get_data(world))
        }
        pub(super) fn get_data_mut(
            world: &mut World,
        ) -> rvimage_domain::RvResult<&mut $crate::tools_data::ToolsData> {
            $crate::world::get_mut(world, $actor_name, $missing_data_msg)
        }
        pub fn get_specific_mut(
            world: &mut World,
        ) -> Option<&mut $crate::tools_data::$data_module::$data_type> {
            $crate::world::get_specific_mut($crate::tools_data::$data_func_mut, get_data_mut(world))
        }
    };
}
#[macro_export]
macro_rules! tools_data_accessors_objects {
    ($actor_name:expr, $missing_data_msg:expr, $data_module:ident, $data_type:ident, $data_func:ident, $data_func_mut:ident) => {
        pub(super) fn get_options(world: &World) -> Option<&$crate::tools_data::$data_module::Options> {
            get_specific(world).map(|d| &d.options)
        }
        pub fn get_options_mut(world: &mut World) -> Option<&mut $crate::tools_data::$data_module::Options> {
            get_specific_mut(world).map(|d| &mut d.options)
        }
        pub(super) fn get_track_changes_str(world: &World) -> Option<&'static str> {
            lazy_static::lazy_static! {
                static ref TRACK_CHANGE_STR: String = $crate::tools::core::make_track_changes_str($actor_name);
            };
            let track_changes =
                get_options(world).map(|o| o.core.track_changes) == Some(true);
            $crate::util::wrap_if(&TRACK_CHANGE_STR, track_changes)
        }

        pub fn get_label_info(world: &World) -> Option<&LabelInfo> {
            get_specific(world).map(|d| &d.label_info)
        }
        pub(super) fn get_instance_label_display(world: &World) -> $crate::tools_data::InstanceLabelDisplay {
            get_options(world).map(|d| d.core.instance_label_display).unwrap_or_default()
        }

        /// when you access annotations, you often also need this metadata
        pub(super) struct DataAccessors;
        impl $crate::world::MetaDataAccess for DataAccessors {
            fn get_core_options(world: &World) -> Option<&$crate::tools_data::Options> {
                get_options(world).map(|o| &o.core)
            }
            fn get_core_options_mut(world: &mut World) -> Option<&mut $crate::tools_data::Options> {
                get_options_mut(world).map(|o| &mut o.core)
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
            fn get_predictive_labeling_data(world: &World) -> Option<&$crate::tools_data::predictive_labeling::PredictiveLabelingData> {
                get_specific(world).map(|d| &d.predictive_labeling_data)
            }
            fn get_predictive_labeling_data_mut(world: &mut World) -> Option<&mut $crate::tools_data::predictive_labeling::PredictiveLabelingData> {
                get_specific_mut(world).map(|d| &mut d.predictive_labeling_data)
            }
        }

        pub(super) fn get_visible(world: &World) -> Visibility {
            let visible = get_options(world).map(|o| o.core.visible) == Some(true);
            vis_from_lfoption(get_label_info(world), visible)
        }
        pub(super) fn set_visible(world: &mut World) {
            let options_mut = get_options_mut(world);
            if let Some(options_mut) = options_mut {
                options_mut.core.visible = true;
            }
            let vis = get_visible(world);
            world.request_redraw_annotations($actor_name, vis);
        }
    };
}
#[macro_export]
macro_rules! world_annotations_accessor {
    ($actor_name:expr, $access_func:ident, $error_msg:expr, $annotations_type:ty) => {
        pub(super) fn get_annos_(
            world: &World,
            is_no_anno_fine: bool,
        ) -> Option<&$annotations_type> {
            if let Some(current_file_path) = world.data.meta_data.file_path_relative() {
                let res = $crate::get_annos_from_tdm!(
                    $actor_name,
                    &world.data.tools_data_map,
                    current_file_path,
                    $access_func
                );
                if res.is_none() && !is_no_anno_fine {
                    tracing::error!("{}", $error_msg);
                }
                res
            } else {
                None
            }
        }
        #[allow(unused)]
        pub fn get_annos(world: &World) -> Option<&$annotations_type> {
            get_annos_(world, false)
        }
        #[allow(unused)]
        pub fn get_annos_if_some(world: &World) -> Option<&$annotations_type> {
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
                let shape_initial = world.shape_orig();
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
        pub fn get_annos_mut(world: &mut World) -> Option<&mut $annotations_type> {
            let is_no_anno_fine = world.data.meta_data.flags.is_file_list_empty == Some(true);
            get_annos_mut_(world, is_no_anno_fine)
        }
        pub fn use_currentimageshape_for_annos(world: &mut World) {
            // we update the shape of the image on each mutable annotation access
            // annotations can be wrong when the image is changed after the annotation has been created
            // or if in case of the attriubte tool the image shape is not necessary and just write dummy
            // numbers are written.
            get_annos_mut(world);
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

#[derive(Clone, Default, PartialEq)]
pub struct DataRaw {
    im_background: DynamicImage,
    ui_image_rect: Option<ShapeF>,
    pub meta_data: MetaData,
    pub tools_data_map: ToolsDataMap,
}

impl DataRaw {
    #[must_use]
    pub fn new(
        im_background: DynamicImage,
        tools_data_map: ToolsDataMap,
        meta_data: MetaData,
        ui_image_rect: Option<ShapeF>,
    ) -> Self {
        DataRaw {
            im_background,
            ui_image_rect,
            meta_data,
            tools_data_map,
        }
    }

    #[must_use]
    pub fn im_background(&self) -> &DynamicImage {
        &self.im_background
    }
    pub fn im_background_mut(&mut self) -> &mut DynamicImage {
        &mut self.im_background
    }

    pub fn set_image_rect(&mut self, ui_image_rect: Option<ShapeF>) {
        self.ui_image_rect = ui_image_rect;
    }

    pub fn apply<FI>(&mut self, mut f_i: FI)
    where
        FI: FnMut(DynamicImage) -> DynamicImage,
    {
        self.im_background = f_i(mem::take(&mut self.im_background));
    }

    #[must_use]
    pub fn shape(&self) -> ShapeI {
        ShapeI::from_im(&self.im_background)
    }

    #[must_use]
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
    #[must_use]
    pub fn new(ims_raw: DataRaw, zoom_box: Option<BbF>) -> Self {
        let update_view = UpdateView::new(&ims_raw, zoom_box);
        let mut world = Self {
            data: ims_raw,
            zoom_box,
            update_view,
        };
        world.data.tools_data_map =
            add_tools_initial_data(mem::take(&mut world.data.tools_data_map));
        world
    }
    pub fn reset_updateview(&mut self) {
        self.update_view = UpdateView::new(&self.data, self.zoom_box);
    }

    #[must_use]
    pub fn ui_image_rect(&self) -> Option<ShapeF> {
        self.data.ui_image_rect
    }

    /// Annotations shall be drawn again
    ///
    /// # Panics
    /// Panics if a tool name is passed that does not have annotations to be redrawn.
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
                let vli = self.data.tools_data_map.get(tool_name_inactive).map(|td| {
                    match &td.specifics {
                        tools_data::ToolSpecifics::Bbox(bbox_data) => {
                            (bbox_data.options.core.visible, bbox_data.label_info())
                        }
                        tools_data::ToolSpecifics::Brush(brush_data) => {
                            (brush_data.options.core.visible, brush_data.label_info())
                        }
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
            self.update_view.image = UpdateImage::Yes(self.data.bg_to_uncropped_view());
        }
    }

    /// real image in contrast to the loading image
    #[must_use]
    pub fn from_real_im(
        im: DynamicImage,
        tools_data: ToolsDataMap,
        ui_image_rect: Option<ShapeF>,
        file_path: Option<String>,
        prj_path: &Path,
        file_selected_idx: Option<usize>,
    ) -> Self {
        let meta_data = match (file_path, file_selected_idx) {
            (Some(fp), Some(fsidx)) => MetaData::from_filepath(fp, fsidx, prj_path),
            _ => MetaData::default(),
        };
        Self::new(DataRaw::new(im, tools_data, meta_data, ui_image_rect), None)
    }

    #[must_use]
    pub fn shape_orig(&self) -> ShapeI {
        self.data.shape()
    }

    pub fn set_zoom_box(&mut self, zoom_box: Option<BbF>) {
        let mut set_zb = || {
            let zoom_box =
                zoom_box.map(|zb| BbF::new_fit_to_image(zb.x, zb.y, zb.w, zb.h, self.shape_orig()));
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

    #[must_use]
    pub fn zoom_box(&self) -> &Option<BbF> {
        &self.zoom_box
    }

    pub fn set_image_rect(&mut self, ui_image_rect: Option<ShapeF>) {
        self.data.set_image_rect(ui_image_rect);
    }
    pub fn set_background_image(&mut self, image: DynamicImage) {
        if ShapeI::from_im(&image) != self.shape_orig() {
            self.zoom_box = None;
        }
        self.data.im_background = image;
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
