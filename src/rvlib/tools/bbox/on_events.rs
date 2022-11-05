use crate::{
    history::Record,
    tools::core::InitialView,
    util::{orig_pos_to_view_pos, shape_unscaled, to_i64, BB},
    {
        history::History,
        util::{mouse_pos_to_orig_pos, Shape},
        world::World,
    },
};

use super::core::{current_cat_id, draw_on_view, get_annos, get_annos_mut, ACTOR_NAME};

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
        let annos = get_annos_mut(&mut world);
        // selection
        let idx =
            mp_orig.and_then(|(x, y)| find_closest_boundary_idx((x as u32, y as u32), annos.bbs()));
        if let Some(i) = idx {
            annos.toggle_selection(i);
        }
        world = draw_on_view(initial_view, are_boxes_visible, world, shape_win);
    } else {
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
    super::core::initialize_tools_menu_data,
    crate::{result::RvResult, types::ViewImage},
    image::DynamicImage,
    std::collections::HashMap,
};

#[test]
fn test_mouse_release() -> RvResult<()> {
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
    let make_params = |prev_pos, is_ctrl_held| MouseReleaseParams {
        prev_pos,
        are_boxes_visible: true,
        is_ctrl_held,
        initial_view: &inital_view,
    };
    {
        let params = make_params(Some((30, 30)), false);
        let (world, _, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos, None);
        let annos = get_annos(&world);
        assert_eq!(annos.bbs().len(), 1);
    }
    {
        let params = make_params(None, false);
        let (world, _, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos, mouse_pos);
        let annos = get_annos(&world);
        assert!(annos.bbs().is_empty());
    }
    {
        let params = make_params(None, true);
        let (world, _, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos, None);
        let annos = get_annos(&world);
        assert!(annos.bbs().is_empty());
    }
    Ok(())
}
#[cfg(test)]
fn make_test_bbs() -> Vec<BB> {
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
