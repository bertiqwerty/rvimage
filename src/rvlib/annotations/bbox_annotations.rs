use super::core::Annotate;
use crate::{
    types::ViewImage,
    util::{self, Shape, BB},
};
use image::{GenericImage, Rgb};

const BBOX_ALPHA: u8 = 90;
const BBOX_ALPHA_SELECTED: u8 = 170;

pub fn _draw_bx_on_image<I: GenericImage, F: Fn(&I::Pixel) -> I::Pixel>(
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

fn draw_bbs<'a, I1: Iterator<Item = &'a BB>, I2: Iterator<Item = &'a bool>>(
    mut im: ViewImage,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
    bbs: I1,
    selected_bbs: I2,
    color: &Rgb<u8>,
) -> ViewImage {
    for (bb, is_selected) in bbs.zip(selected_bbs) {
        let alpha = if *is_selected {
            BBOX_ALPHA_SELECTED
        } else {
            BBOX_ALPHA
        };
        let f_inner_color = |rgb: &Rgb<u8>| util::apply_alpha(rgb, color, alpha);
        let view_corners = bb.to_view_corners(shape_orig, shape_win, zoom_box);
        im = util::draw_bx_on_image(im, view_corners.0, view_corners.1, color, f_inner_color);
    }
    im
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BboxAnnotations {
    pub bbs: Vec<BB>,
    pub selected_bbs: Vec<bool>,
}
impl Annotate for BboxAnnotations {
    fn draw_on_view(
        &self,
        im_view: ViewImage,
        zoom_box: &Option<BB>,
        shape_orig: Shape,
        shape_win: Shape,
    ) -> ViewImage {
        draw_bbs(
            im_view,
            shape_orig,
            shape_win,
            zoom_box,
            self.bbs.iter(),
            self.selected_bbs.iter(),
            &Rgb([255, 255, 255]),
        )
    }
}
