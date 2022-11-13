use crate::{
    domain::{mouse_pos_to_orig_pos, orig_pos_to_view_pos, shape_unscaled, Shape, BB},
    file_util::MetaData,
    history::Record,
    image_util::to_i64,
    tools::core::{InitialView, Mover},
    tools_data::{bbox_data::BboxExportFileType, BboxSpecificData},
    {history::History, world::World},
};

use super::{
    core::{current_cat_id, draw_on_view, get_annos, get_annos_mut, ACTOR_NAME},
    io,
};

const CORNER_TOL_DENOMINATOR: u32 = 5000;

fn find_closest_boundary_idx(pos: (u32, u32), bbs: &[BB]) -> Option<usize> {
    bbs.iter()
        .enumerate()
        .filter(|(_, bb)| bb.contains(pos))
        .map(|(i, bb)| {
            let dx = (bb.x as i64 - pos.0 as i64).abs();
            let dw = ((bb.x + bb.w) as i64 - pos.0 as i64).abs();
            let dy = (bb.y as i64 - pos.1 as i64).abs();
            let dh = ((bb.y + bb.h) as i64 - pos.1 as i64).abs();
            (i, dx.min(dw).min(dy).min(dh))
        })
        .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap())
        .map(|(i, _)| i)
}

/// returns index of the bounding box and the index of the closest close corner
fn find_close_corner(orig_pos: (u32, u32), bbs: &[BB], tolerance: i64) -> Option<(usize, usize)> {
    let opf = to_i64(orig_pos);
    bbs.iter()
        .enumerate()
        .map(|(bb_idx, bb)| {
            let (min_corner_idx, min_corner_dist) = bb
                .corners()
                .map(to_i64)
                .map(|c| (opf.0 - c.0).pow(2) + (opf.1 - c.1).pow(2))
                .enumerate()
                .min_by_key(|x| x.1)
                .unwrap();
            (bb_idx, min_corner_idx, min_corner_dist)
        })
        .filter(|(_, _, c_dist)| c_dist <= &tolerance)
        .min_by_key(|(_, _, c_dist)| *c_dist)
        .map(|(bb_idx, c_idx, _)| (bb_idx, c_idx))
}

pub(super) fn export_if_triggered(meta_data: &MetaData, bbox_data: BboxSpecificData) {
    match bbox_data.export_file_type {
        // TODO: don't crash just because export failed
        BboxExportFileType::Json => {
            io::write_json(meta_data, bbox_data).unwrap();
        }
        BboxExportFileType::Pickle => {
            io::write_pickle(meta_data, bbox_data).unwrap();
        }
        BboxExportFileType::None => (),
    };
}

pub(super) struct MouseHeldParams<'a> {
    pub are_boxes_visible: bool,
    pub initial_view: &'a InitialView,
    pub mover: &'a mut Mover,
}
pub(super) fn on_mouse_held_right(
    shape_win: Shape,
    mouse_pos: Option<(usize, usize)>,
    params: MouseHeldParams,
    mut world: World,
    mut history: History,
) -> (World, History) {
    let orig_shape = world.data.shape();
    let zoom_box = *world.zoom_box();
    let mut add_to_history = false;
    let move_boxes = |mpso, mpo| {
        let annos = get_annos_mut(&mut world);
        add_to_history = annos.selected_follow_movement(mpso, mpo, orig_shape);
        Some(())
    };
    params
        .mover
        .move_mouse_held(move_boxes, mouse_pos, shape_win, orig_shape, &zoom_box);
    world = draw_on_view(
        params.initial_view,
        params.are_boxes_visible,
        world,
        shape_win,
    );
    if add_to_history {
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
    }
    (world, history)
}

pub(super) struct MouseReleaseParams<'a> {
    pub prev_pos: Option<(usize, usize)>,
    pub are_boxes_visible: bool,
    pub is_ctrl_held: bool,
    pub initial_view: &'a InitialView,
}

pub(super) fn on_mouse_released_left(
    shape_win: Shape,
    mouse_pos: Option<(usize, usize)>,
    params: MouseReleaseParams,
    mut world: World,
    mut history: History,
) -> (World, History, Option<(usize, usize)>) {
    let MouseReleaseParams {
        mut prev_pos,
        are_boxes_visible,
        is_ctrl_held,
        initial_view,
    } = params;
    let mp_orig = mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, world.zoom_box());
    let pp_orig = mouse_pos_to_orig_pos(prev_pos, world.shape_orig(), shape_win, world.zoom_box());
    let in_menu_selected_label = current_cat_id(&world);
    if let (Some(mp), Some(pp)) = (mp_orig, pp_orig) {
        // second click new bb
        if (mp.0 as i32 - pp.0 as i32).abs() > 1 && (mp.1 as i32 - pp.1 as i32).abs() > 1 {
            let annos = get_annos_mut(&mut world);
            annos.add_bb(BB::from_points(mp, pp), in_menu_selected_label);
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
            prev_pos = None;
            world = draw_on_view(initial_view, are_boxes_visible, world, shape_win);
        }
    } else if is_ctrl_held {
        // selection
        let annos = get_annos_mut(&mut world);
        let idx =
            mp_orig.and_then(|(x, y)| find_closest_boundary_idx((x as u32, y as u32), annos.bbs()));
        if let Some(i) = idx {
            annos.toggle_selection(i);
        }
        world = draw_on_view(initial_view, are_boxes_visible, world, shape_win);
    } else {
        // first click defines starting point of bounding box
        let shape_orig = world.data.shape();
        let unscaled = shape_unscaled(world.zoom_box(), shape_orig);
        let tolerance = (unscaled.w * unscaled.h / CORNER_TOL_DENOMINATOR).max(2);
        let close_corner =
            mp_orig.and_then(|mp| find_close_corner(mp, get_annos(&world).bbs(), tolerance as i64));
        if let Some((bb_idx, idx)) = close_corner {
            // move an existing corner
            let annos = get_annos_mut(&mut world);
            let bb = annos.remove(bb_idx);
            let oppo_corner = bb.opposite_corner(idx);
            prev_pos = orig_pos_to_view_pos(oppo_corner, shape_orig, shape_win, world.zoom_box())
                .map(|(x, y)| (x as usize, y as usize));
        } else {
            // first click new bb
            prev_pos = mouse_pos;
        }
    }
    (world, history, prev_pos)
}
#[cfg(test)]
use {
    super::core::initialize_tools_menu_data, super::core::make_test_bbs, crate::types::ViewImage,
    image::DynamicImage, std::collections::HashMap,
};
#[cfg(test)]
fn test_data() -> (InitialView, Option<(usize, usize)>, Shape, World, History) {
    let im_test = DynamicImage::ImageRgb8(ViewImage::new(64, 64));
    let shape_win = Shape { w: 64, h: 64 };
    let world = World::from_real_im(
        im_test,
        HashMap::new(),
        "superimage.png".to_string(),
        shape_win,
    );
    let world = initialize_tools_menu_data(world);
    let history = History::new();
    let mut inital_view = InitialView::new();
    inital_view.update(&world, shape_win);
    let mouse_pos = Some((32, 32));
    (inital_view, mouse_pos, shape_win, world, history)
}
#[cfg(test)]
fn history_equal(hist1: &History, hist2: &History) -> bool {
    format!("{:?}", hist1) == format!("{:?}", hist2)
}
#[test]
fn test_mouse_held() {
    let (initial_view, mouse_pos, shape_win, mut world, history) = test_data();
    let annos = get_annos_mut(&mut world);
    let bbs = make_test_bbs();
    annos.add_bb(bbs[0].clone(), 0);
    {
        let mut mover = Mover::new();
        mover.move_mouse_pressed(Some((12, 12)));
        let params = MouseHeldParams {
            are_boxes_visible: true,
            initial_view: &initial_view,
            mover: &mut mover,
        };
        let (world, new_hist) =
            on_mouse_held_right(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(get_annos(&world).bbs()[0], bbs[0]);
        assert!(history_equal(&history, &new_hist));
    }
    {
        let mut mover = Mover::new();
        mover.move_mouse_pressed(Some((12, 12)));
        let params = MouseHeldParams {
            are_boxes_visible: true,
            initial_view: &initial_view,
            mover: &mut mover,
        };
        let annos = get_annos_mut(&mut world);
        annos.select(0);
        let (world, new_hist) =
            on_mouse_held_right(shape_win, mouse_pos, params, world, history.clone());
        assert_ne!(get_annos(&world).bbs()[0], bbs[0]);
        assert!(!history_equal(&history, &new_hist));
    }
}
#[test]
fn test_mouse_release() {
    let (initial_view, mouse_pos, shape_win, world, history) = test_data();
    let make_params = |prev_pos, is_ctrl_held| MouseReleaseParams {
        prev_pos,
        are_boxes_visible: true,
        is_ctrl_held,
        initial_view: &initial_view,
    };
    {
        // If a previous position was registered, we expect that the second click creates the
        // bounding box.
        let params = make_params(Some((30, 30)), false);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos, None);
        let annos = get_annos(&world);
        assert_eq!(annos.bbs().len(), 1);
        assert!(format!("{:?}", new_hist).len() > format!("{:?}", history).len());
    }
    {
        // If no position was registered, a left click will set trigger the start
        // of defining a new bounding box. The other corner will be defined by a second click.
        let params = make_params(None, false);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos, mouse_pos);
        let annos = get_annos(&world);
        assert!(annos.bbs().is_empty());
        assert!(history_equal(&history, &new_hist));
    }
    {
        // If ctrl is hold, a bounding box would be selected. Since no bounding boxes are selected,
        // nothing should happen.
        let params = make_params(None, true);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos, None);
        let annos = get_annos(&world);
        assert!(annos.bbs().is_empty());
        assert!(history_equal(&history, &new_hist));
    }
    {
        // If ctrl is hold at the second click, this does not really make sense. We ignore it and assume this
        // is the finishing box click.
        let params = make_params(Some((30, 30)), true);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos, None);
        let annos = get_annos(&world);
        assert_eq!(annos.bbs().len(), 1);
        assert!(format!("{:?}", new_hist).len() > format!("{:?}", history).len());
    }
}
#[test]
fn test_find_idx() {
    let bbs = make_test_bbs();
    assert_eq!(find_closest_boundary_idx((0, 20), &bbs), None);
    assert_eq!(find_closest_boundary_idx((0, 0), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((3, 8), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((7, 14), &bbs), Some(1));
    assert_eq!(find_closest_boundary_idx((7, 15), &bbs), None);
    assert_eq!(find_closest_boundary_idx((8, 8), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((10, 12), &bbs), Some(2));
}
