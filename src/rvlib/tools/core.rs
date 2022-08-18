use image::{GenericImage, Rgb};
use winit_input_helper::WinitInputHelper;

use crate::{
    history::History,
    types::ViewImage,
    util::{self, Shape, BB},
    world::World,
};

pub struct MetaData<'a> {
    pub file_path: Option<&'a str>,
}

pub trait Manipulate {
    fn new() -> Self
    where
        Self: Sized;

    fn on_deactivate(&mut self) {}
    /// All events that are used by a tool are implemented in here. Use the macro [`make_tool_transform`](make_tool_transform). See, e.g.,
    /// [`Zoom::events_tf`](crate::tools::Zoom::events_tf).
    fn events_tf(
        &mut self,
        world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        input_event: &WinitInputHelper,
        meta_data: &MetaData,
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
    draw_bx_on_image(im, Some(corner_1), Some(corner_2), color, f)
}

pub fn draw_bx_on_image<I: GenericImage, F: Fn(&I::Pixel) -> I::Pixel>(
    mut im: I,
    corner_1: Option<(u32, u32)>,
    corner_2: Option<(u32, u32)>,
    color: &I::Pixel,
    fn_inner_color: F,
) -> I {
    if corner_1.is_none() && corner_2.is_none() {
        return im;
    }
    let (x_min, y_min) = corner_1.unwrap_or((0, 0));
    let (x_max, y_max) = corner_2.unwrap_or((im.width(), im.height()));
    let draw_bx = BB {
        x: x_min as u32,
        y: y_min as u32,
        w: (x_max - x_min) as u32,
        h: (y_max - y_min) as u32,
    };

    let inner_effect = |x, y, im: &mut I| {
        let rgb = im.get_pixel(x, y);
        im.put_pixel(x, y, fn_inner_color(&rgb));
    };
    {
        let mut put_pixel = |c: Option<(u32, u32)>, x, y| {
            if c.is_some() {
                im.put_pixel(x, y, *color);
            } else {
                inner_effect(x, y, &mut im);
            }
        };
        for x in draw_bx.x_range() {
            put_pixel(corner_1, x, draw_bx.y);
            put_pixel(corner_2, x, draw_bx.y + draw_bx.h - 1);
        }
        for y in draw_bx.y_range() {
            put_pixel(corner_1, draw_bx.x, y);
            put_pixel(corner_2, draw_bx.x + draw_bx.w - 1, y);
        }
    }
    draw_bx.effect_per_inner_pixel(|x, y| inner_effect(x, y, &mut im));
    im
}
