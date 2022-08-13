
use image::{GenericImage, Pixel, Rgb, Rgba};
use winit_input_helper::WinitInputHelper;

use crate::{
    history::History,
    types::ViewImage,
    util::{self, Shape, BB},
    world::{AnnotationImage, World},
};

pub trait Manipulate {
    fn new() -> Self
    where
        Self: Sized;

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

pub fn draw_bx_on_anno(
    im: AnnotationImage,
    corner_1: (usize, usize),
    corner_2: (usize, usize),
    color: Rgb<u8>,
    alpha: u8,
) -> AnnotationImage {
    let f = |Rgba([r, g, b, a]): Rgba<u8>| {
        Rgba([
            color[0].max(r),
            color[1].max(g),
            color[2].max(b),
            alpha.max(a),
        ])
    };
    draw_bx_on_image(im, corner_1, corner_2, color.to_rgba(), f)
}

pub fn draw_bx_on_view(
    im: ViewImage,
    corner_1: (usize, usize),
    corner_2: (usize, usize),
    color: Rgb<u8>,
) -> ViewImage {
    let offset = Rgb([color[0] / 5, color[1] / 5, color[2] / 5]);
    let f = |rgb: Rgb<u8>| {
        Rgb([
            util::clipped_add(offset[0], rgb[0], 255),
            util::clipped_add(offset[1], rgb[1], 255),
            util::clipped_add(offset[2], rgb[2], 255),
        ])
    };
    draw_bx_on_image(im, corner_1, corner_2, color, f)
}

pub fn draw_bx_on_image<I: GenericImage, F: Fn(I::Pixel) -> I::Pixel>(
    mut im: I,
    corner_1: (usize, usize),
    corner_2: (usize, usize),
    color: I::Pixel,
    fn_inner_color: F,
) -> I {
    let (p1_x, p1_y) = corner_1;
    let (p2_x, p2_y) = corner_2;
    let x_min = p1_x.min(p2_x);
    let y_min = p1_y.min(p2_y);
    let x_max = p1_x.max(p2_x);
    let y_max = p1_y.max(p2_y);
    let draw_bx = BB {
        x: x_min as u32,
        y: y_min as u32,
        w: (x_max - x_min) as u32,
        h: (y_max - y_min) as u32,
    };
    for x in draw_bx.x_range() {
        im.put_pixel(x, draw_bx.y, color);
        im.put_pixel(x, draw_bx.y + draw_bx.h - 1, color);
    }
    for y in draw_bx.y_range() {
        im.put_pixel(draw_bx.x, y, color);
        im.put_pixel(draw_bx.x + draw_bx.w - 1, y, color);
    }
    draw_bx.effect_per_inner_pixel(|x, y| {
        let rgb = im.get_pixel(x, y);
        im.put_pixel(x, y, fn_inner_color(rgb));
    });
    im
}
