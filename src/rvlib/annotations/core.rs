use crate::{
    types::ViewImage,
    util::{Shape, BB},
};

pub trait Annotate {
    fn draw_on_view(
        &self,
        im_view: ViewImage,
        zoom_box: &Option<BB>,
        shape_orig: Shape,
        shape_win: Shape,
    ) -> ViewImage;
}
