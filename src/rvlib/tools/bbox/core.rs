use crate::{
    annotations::BboxAnnotations,
    annotations_accessor, annotations_accessor_mut,
    tools::core::InitialView,
    tools_data::{BboxSpecificData, ToolSpecifics, ToolsData},
    tools_data_accessor, tools_data_accessor_mut, tools_data_initializer,
    util::Shape,
    world::World,
};

pub const ACTOR_NAME: &str = "BBox";
const MISSING_ANNO_MSG: &str = "bbox annotations have not yet been initialized";
const MISSING_TOOLSMENU_MSG: &str = "bbox tools menu has not yet been initialized";
tools_data_initializer!(ACTOR_NAME, Bbox, BboxSpecificData);
tools_data_accessor!(ACTOR_NAME, MISSING_TOOLSMENU_MSG);
tools_data_accessor_mut!(ACTOR_NAME, MISSING_TOOLSMENU_MSG);
annotations_accessor_mut!(ACTOR_NAME, bbox_mut, MISSING_ANNO_MSG, BboxAnnotations);
annotations_accessor!(ACTOR_NAME, bbox, MISSING_ANNO_MSG, BboxAnnotations);

pub(super) fn current_cat_id(world: &World) -> usize {
    get_tools_data(world).specifics.bbox().cat_id_current
}

pub(super) fn draw_on_view(
    initial_view: &InitialView,
    are_boxes_visible: bool,
    mut world: World,
    shape_win: Shape,
) -> World {
    if are_boxes_visible {
        let bb_data = &get_tools_data(&world).specifics.bbox();
        let im_view = get_annos(&world).draw_on_view(
            initial_view.image().clone().unwrap(),
            world.zoom_box(),
            world.data.shape(),
            shape_win,
            bb_data.labels(),
            bb_data.colors(),
        );
        world.set_im_view(im_view);
    } else if let Some(iv) = initial_view.image() {
        world.set_im_view(iv.clone());
    }
    world
}

#[cfg(test)]
use crate::util::BB;

#[cfg(test)]
pub(super) fn make_test_bbs() -> Vec<BB> {
    vec![
        BB {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        },
        BB {
            x: 5,
            y: 5,
            w: 10,
            h: 10,
        },
        BB {
            x: 9,
            y: 9,
            w: 10,
            h: 10,
        },
    ]
}
