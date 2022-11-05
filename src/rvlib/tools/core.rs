use image::Rgb;
use winit_input_helper::WinitInputHelper;

use crate::{
    cfg::SshCfg,
    history::History,
    types::ViewImage,
    util::{self, Shape, BB},
    world::World,
};

#[derive(Debug, Clone)]
pub struct InitialView {
    file_path: Option<String>,
    image: Option<ViewImage>,
}
impl InitialView {
    pub fn update(&mut self, world: &World, shape_win: Shape) {
        if self.file_path != world.data.meta_data.file_path
            || (self.file_path.is_some() && self.image.is_none())
        {
            self.file_path = world
                .data
                .meta_data
                .file_path
                .as_ref()
                .map(|s| s.to_string());
            self.image = Some(
                world
                    .data
                    .bg_to_unannotated_view(world.zoom_box(), shape_win),
            );
        }
    }
    pub fn image(&self) -> &Option<ViewImage> {
        &self.image
    }
    pub fn new() -> Self {
        Self {
            file_path: None,
            image: None,
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct MetaData {
    pub file_path: Option<String>,
    pub ssh_cfg: Option<SshCfg>,
    pub open_folder: Option<String>,
}
impl MetaData {
    pub fn from_filepath(file_path: String) -> Self {
        MetaData {
            file_path: Some(file_path),
            ssh_cfg: None,
            open_folder: None,
        }
    }
}

pub trait Manipulate {
    fn new() -> Self
    where
        Self: Sized;

    fn on_activate(
        &mut self,
        world: World,
        history: History,
        _shape_win: Shape,
    ) -> (World, History) {
        (world, history)
    }
    fn on_deactivate(
        &mut self,
        world: World,
        history: History,
        _shape_win: Shape,
    ) -> (World, History) {
        (world, history)
    }
    /// All events that are used by a tool are implemented in here. Use the macro [`make_tool_transform`](make_tool_transform). See, e.g.,
    /// [`Zoom::events_tf`](crate::tools::Zoom::events_tf).
    fn events_tf(
        &mut self,
        world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        input_event: &WinitInputHelper,
    ) -> (World, History);
}

#[derive(Clone, Debug)]
pub struct Mover {
    mouse_pos_start: Option<(usize, usize)>,
}
impl Mover {
    pub fn new() -> Self {
        Self {
            mouse_pos_start: None,
        }
    }
    pub fn move_mouse_held<T, F: FnOnce((u32, u32), (u32, u32)) -> T>(
        &mut self,
        f_move: F,
        mouse_pos: Option<(usize, usize)>,
        shape_win: Shape,
        shape_orig: Shape,
        zoom_box: &Option<BB>,
    ) -> Option<T> {
        let res = if let (Some(mps), Some(mp)) = (self.mouse_pos_start, mouse_pos) {
            let mps_orig = util::mouse_pos_to_orig_pos(Some(mps), shape_orig, shape_win, zoom_box);
            let mp_orig = util::mouse_pos_to_orig_pos(Some(mp), shape_orig, shape_win, zoom_box);
            match (mps_orig, mp_orig) {
                (Some(mpso), Some(mpo)) => Some(f_move(mpso, mpo)),
                _ => None,
            }
        } else {
            None
        };
        self.mouse_pos_start = mouse_pos;
        res
    }
    pub fn move_mouse_pressed(&mut self, mouse_pos: Option<(usize, usize)>) {
        if mouse_pos.is_some() {
            self.mouse_pos_start = mouse_pos;
        }
    }
}

// applies the tool transformation to the world
#[macro_export]
macro_rules! make_tool_transform {
    (
        $self:expr,
        $world:expr,
        $history:expr,
        $shape_win:expr,
        $mouse_pos:expr,
        $event:expr,
        [$(($mouse_event:ident, $mouse_btn:expr)),*],
        [$(($key_event:ident, $key_btn:expr)),*]
    ) => {
        if false {
            ($world, $history)
        }
        $(else if $event.$mouse_event($mouse_btn) {
            $self.$mouse_event($event, $shape_win, $mouse_pos, $world, $history)
        })*
        $(else if $event.$key_event($key_btn) {
            $self.$key_event($event, $shape_win, $mouse_pos, $world, $history)
        })*
        else {
            ($world, $history)
        }
    };
}

pub fn draw_bx_on_view(
    im: ViewImage,
    corner_1: (u32, u32),
    corner_2: (u32, u32),
    color: &Rgb<u8>,
) -> ViewImage {
    let offset = Rgb([color[0] / 5, color[1] / 5, color[2] / 5]);
    let f = |rgb: &Rgb<u8>| {
        Rgb([
            util::clipped_add(offset[0], rgb[0], 255),
            util::clipped_add(offset[1], rgb[1], 255),
            util::clipped_add(offset[2], rgb[2], 255),
        ])
    };
    util::draw_bx_on_image(
        im,
        (Some(corner_1.0), Some(corner_1.1)),
        (Some(corner_2.0), Some(corner_2.1)),
        color,
        f,
    )
}
