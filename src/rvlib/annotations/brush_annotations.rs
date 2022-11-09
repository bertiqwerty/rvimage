use image::Rgb;
use imageproc::drawing;

use crate::{
    domain::{orig_pos_to_view_pos, Shape, BB},
    types::ViewImage,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrushAnnotations {
    pub points: Vec<Vec<(u32, u32)>>,
    pub color: [u8; 3],
}
impl BrushAnnotations {
    pub fn draw_on_view(
        &self,
        mut im_view: ViewImage,
        zoom_box: &Option<BB>,
        shape_orig: Shape,
        shape_win: Shape,
    ) -> ViewImage {
        let clr = Rgb::<u8>(self.color);
        for points in &self.points {
            if !self.points.is_empty() {
                let view_points = points
                    .iter()
                    .flat_map(|op| orig_pos_to_view_pos(*op, shape_orig, shape_win, zoom_box));
                let mut advanced = view_points.clone();
                advanced.next();
                for (p, p_next) in view_points.zip(advanced) {
                    let start = (p.0 as f32, p.1 as f32);
                    let end = (p_next.0 as f32, p_next.1 as f32);
                    drawing::draw_line_segment_mut(&mut im_view, start, end, clr);
                }
            }
        }
        im_view
    }
}
