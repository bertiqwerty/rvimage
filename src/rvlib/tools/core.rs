use image::{GenericImage, Rgb, Rgba};
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

// applies the tool transformation to the world
#[macro_export]
macro_rules! make_tool_transform {
    ($self:expr, $world:expr, $history:expr, $shape_win:expr, $mouse_pos:expr, $event:expr, [$($mouse_event:ident),*], [$($key_event:expr),*]) => {
        if false {
            ($world, $history)
        }
        $(else if $event.$mouse_event(LEFT_BTN) {
            $self.$mouse_event(LEFT_BTN, $shape_win, $mouse_pos, $world, $history)
        } else if $event.$mouse_event(RIGHT_BTN) {
            $self.$mouse_event(RIGHT_BTN, $shape_win, $mouse_pos, $world, $history)
        })*
        $(else if $event.key_pressed($key_event) {
            $self.key_pressed($key_event, $shape_win, $mouse_pos, $world, $history)
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
    color: Rgba<u8>,
) -> AnnotationImage {
    let offset = Rgba([color[0] / 5, color[1] / 5, color[2] / 5, color[3] / 5]);
    let f = |rgba: Rgba<u8>| {
        Rgba([
            util::clipped_add(offset[0], rgba[0], color[0]),
            util::clipped_add(offset[1], rgba[1], color[1]),
            util::clipped_add(offset[2], rgba[2], color[2]),
            util::clipped_add(offset[3], rgba[3], color[3]),
        ])
    };
    draw_bx_on_image(im, corner_1, corner_2, color, f)
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
            util::clipped_add(offset[0], rgb[0], color[0]),
            util::clipped_add(offset[1], rgb[1], color[1]),
            util::clipped_add(offset[2], rgb[2], color[2]),
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
