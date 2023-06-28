use crate::{
    annotations::selected_indices,
    domain::{
        mouse_pos_to_orig_pos, orig_pos_to_view_pos, shape_unscaled, OutOfBoundsMode, Shape, BB,
    },
    file_util::MetaData,
    history::Record,
    image_util::to_i64,
    tools::core::{InitialView, Mover},
    tools_data::{
        self,
        bbox_data::{ClipboardData, SplitMode},
        BboxSpecificData,
    },
    {history::History, world::World},
};
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::core::{
    current_cat_idx, draw_on_view, get_annos, get_annos_mut, get_tools_data, get_tools_data_mut,
    paste, ACTOR_NAME,
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

pub(super) fn import_coco_if_triggered(
    meta_data: &MetaData,
    is_coco_import_triggered: bool,
) -> Option<BboxSpecificData> {
    if is_coco_import_triggered {
        match tools_data::coco_io::read_coco(meta_data) {
            Ok(bbox_data) => Some(bbox_data),
            Err(e) => {
                println!("could not import coco due to {e:?}");
                None
            }
        }
    } else {
        None
    }
}

pub(super) fn export_if_triggered(meta_data: &MetaData, bbox_data: &BboxSpecificData) {
    if bbox_data.options.is_export_triggered {
        // TODO: don't crash just because export failed
        tools_data::write_coco(meta_data, bbox_data.clone()).unwrap();
    }
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
    let move_boxes = |mpo_from, mpo_to| {
        let split_mode = get_tools_data(&world).specifics.bbox().options.split_mode;
        let annos = get_annos_mut(&mut world);
        add_to_history = match split_mode {
            SplitMode::None => annos.selected_follow_movement(
                mpo_from,
                mpo_to,
                orig_shape,
                OutOfBoundsMode::Deny,
                split_mode,
            ),
            SplitMode::Horizontal => {
                let min_shape = Shape::new(1, 30);
                let mpo_to = (mpo_from.0, mpo_to.1);
                annos.selected_follow_movement(
                    mpo_from,
                    mpo_to,
                    orig_shape,
                    OutOfBoundsMode::Resize(min_shape),
                    split_mode,
                )
            }
            SplitMode::Vertical => {
                let min_shape = Shape::new(30, 1);
                let mpo_to = (mpo_to.0, mpo_from.1);
                annos.selected_follow_movement(
                    mpo_from,
                    mpo_to,
                    orig_shape,
                    OutOfBoundsMode::Resize(min_shape),
                    split_mode,
                )
            }
        };
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

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PrevPos {
    pub prev_pos: Option<(usize, usize)>,
    pub last_valid_click: Option<(usize, usize)>,
}

pub(super) struct MouseReleaseParams<'a> {
    pub prev_pos: PrevPos,

    pub are_boxes_visible: bool,
    pub is_alt_held: bool,
    pub is_shift_held: bool,
    pub is_ctrl_held: bool,
    pub initial_view: &'a InitialView,
}

pub(super) fn on_mouse_released_left(
    shape_win: Shape,
    mouse_pos: Option<(usize, usize)>,
    params: MouseReleaseParams,
    mut world: World,
    mut history: History,
) -> (World, History, PrevPos) {
    let split_mode = get_tools_data(&world).specifics.bbox().options.split_mode;
    let MouseReleaseParams {
        mut prev_pos,
        are_boxes_visible,
        is_alt_held,
        is_shift_held,
        is_ctrl_held,
        initial_view,
    } = params;
    let to_orig_pos =
        |pos| mouse_pos_to_orig_pos(pos, world.shape_orig(), shape_win, world.zoom_box());
    let mp_orig = to_orig_pos(mouse_pos);
    let lc_orig = to_orig_pos(prev_pos.last_valid_click);
    let pp_orig = to_orig_pos(prev_pos.prev_pos);
    let in_menu_selected_label = current_cat_idx(&world);
    if let Some(mp) = mouse_pos {
        prev_pos.last_valid_click = Some(mp);
    }
    if let (Some(mp), Some(pp), Some(last_click)) = (mp_orig, pp_orig, lc_orig) {
        // second click new bb
        if (mp.0 as i32 - pp.0 as i32).abs() > 1 && (mp.1 as i32 - pp.1 as i32).abs() > 1 {
            let mp = match split_mode {
                SplitMode::Horizontal => (last_click.0, mp.1),
                SplitMode::Vertical => (mp.0, last_click.1),
                SplitMode::None => mp,
            };
            let annos = get_annos_mut(&mut world);
            annos.add_bb(BB::from_points(mp, pp), in_menu_selected_label);
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
            prev_pos.prev_pos = None;
            world = draw_on_view(initial_view, are_boxes_visible, world, shape_win);
        }
    } else if is_ctrl_held || is_alt_held || is_shift_held {
        // selection
        let annos = get_annos_mut(&mut world);
        let idx = mp_orig.and_then(|(x, y)| find_closest_boundary_idx((x, y), annos.bbs()));
        if let Some(i) = idx {
            if is_shift_held {
                // If shift is held a new selection box will be spanned between the currently clicked
                // box and the selected box that has the maximum distance in terms of max-corner-dist.
                // All boxes that have overlap with this new selection box will be selected. If no box
                // is selected only the currently clicked box will be selected.
                annos.select(i);
                let newly_selected_bb = &annos.bbs()[i];
                let sel_indxs = selected_indices(annos.selected_bbs());
                if let Some((bbidx, (csidx, coidx, _))) = sel_indxs
                    .map(|i| (i, newly_selected_bb.max_corner_squaredist(&annos.bbs()[i])))
                    .max_by_key(|(_, (_, _, d))| *d)
                {
                    let spanned_bb = BB::from_points(
                        newly_selected_bb.corner(csidx),
                        annos.bbs()[bbidx].corner(coidx),
                    );
                    let to_be_selected_inds = annos
                        .bbs()
                        .iter()
                        .enumerate()
                        .filter(|(_, bb)| bb.has_overlap(&spanned_bb))
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>();
                    annos.select_multi(to_be_selected_inds.iter().copied());
                }
            } else if is_alt_held {
                annos.deselect_all();
                annos.select(i);
                annos.label_selected(in_menu_selected_label);
            } else {
                // ctrl
                annos.toggle_selection(i);
            }
        }
        world = draw_on_view(initial_view, are_boxes_visible, world, shape_win);
    } else {
        let shape_orig = world.data.shape();
        let unscaled = shape_unscaled(world.zoom_box(), shape_orig);
        let tolerance = (unscaled.w * unscaled.h / CORNER_TOL_DENOMINATOR).max(2);
        let close_corner = mp_orig.and_then(|mp| {
            get_annos(&world).and_then(|a| find_close_corner(mp, a.bbs(), tolerance as i64))
        });
        if let Some((bb_idx, idx)) = close_corner {
            // move an existing corner
            let annos = get_annos_mut(&mut world);
            let bb = annos.remove(bb_idx);
            let oppo_corner = bb.opposite_corner(idx);
            prev_pos.prev_pos =
                orig_pos_to_view_pos(oppo_corner, shape_orig, shape_win, world.zoom_box())
                    .map(|(x, y)| (x as usize, y as usize));
        } else {
            match split_mode {
                SplitMode::None => {
                    // first click new bb
                    prev_pos.prev_pos = mouse_pos;
                }
                _ => {
                    // create boxes by splitting either horizontally or vertically
                    if let Some(mp) = mp_orig {
                        let existing_bbs: &[BB] = if let Some(annos) = get_annos(&world) {
                            annos.bbs()
                        } else {
                            &[]
                        };
                        let (x, y) = mp;
                        let new_bbs = if let SplitMode::Horizontal = split_mode {
                            if let Some((i, bb)) = existing_bbs
                                .iter()
                                .enumerate()
                                .find(|(_, bb)| bb.contains((x, y)))
                            {
                                let (top, btm) = bb.split_horizontally(y);
                                vec![(Some(i), top, btm)]
                            } else {
                                let new_bbs = existing_bbs
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, bb)| bb.covers_y(y))
                                    .map(|(i, bb)| {
                                        let (top, btm) = bb.split_horizontally(y);
                                        (Some(i), top, btm)
                                    })
                                    .collect::<Vec<_>>();
                                if new_bbs.is_empty() {
                                    let (top, btm) =
                                        BB::from_shape(shape_orig).split_horizontally(y);
                                    vec![(None, top, btm)]
                                } else {
                                    new_bbs
                                }
                            }
                        // SplitMode::Vertical
                        } else if let Some((i, bb)) = existing_bbs
                            .iter()
                            .enumerate()
                            .find(|(_, bb)| bb.contains((x, y)))
                        {
                            let (left, right) = bb.split_vertically(x);
                            vec![(Some(i), left, right)]
                        } else {
                            let new_bbs = existing_bbs
                                .iter()
                                .enumerate()
                                .filter(|(_, bb)| bb.covers_x(x))
                                .map(|(i, bb)| {
                                    let (left, right) = bb.split_vertically(x);
                                    (Some(i), left, right)
                                })
                                .collect::<Vec<_>>();
                            if new_bbs.is_empty() {
                                let (left, right) = BB::from_shape(shape_orig).split_vertically(x);
                                vec![(None, left, right)]
                            } else {
                                new_bbs
                            }
                        };
                        let annos = get_annos_mut(&mut world);
                        let removers = new_bbs.iter().flat_map(|(i, _, _)| *i).collect::<Vec<_>>();
                        annos.remove_multiple(&removers);
                        for (_, bb1, bb2) in new_bbs {
                            annos.add_bb(bb1, in_menu_selected_label);
                            annos.add_bb(bb2, in_menu_selected_label);
                        }
                        history.push(Record::new(world.data.clone(), ACTOR_NAME));
                        prev_pos.prev_pos = None;
                        world = draw_on_view(initial_view, are_boxes_visible, world, shape_win);
                    }
                }
            }
        }
    }
    (world, history, prev_pos)
}

macro_rules! released_key {
    ($($key:ident),*) => {
        pub(super) enum ReleasedKey {
            None,
            $($key,)*
        }
        pub(super) fn map_released_key(event: &WinitInputHelper) -> ReleasedKey {
            if false {
                ReleasedKey::None
            } $(else if event.key_released(VirtualKeyCode::$key) {
                ReleasedKey::$key
            })*
            else {
                ReleasedKey::None
            }
        }
    };
}

released_key!(A, D, H, C, V, L, Delete, Left, Right, Up, Down);

pub(super) struct KeyReleasedParams<'a> {
    pub initial_view: &'a InitialView,
    pub is_ctrl_held: bool,
    pub released_key: ReleasedKey,
}

pub(super) fn on_key_released(
    mut world: World,
    mut history: History,
    mouse_pos: Option<(usize, usize)>,
    shape_win: Shape,
    params: KeyReleasedParams,
) -> (World, History) {
    let mut flags = get_tools_data_mut(&mut world).specifics.bbox_mut().options;
    match params.released_key {
        ReleasedKey::H if params.is_ctrl_held => {
            // Hide all boxes (selected or not)
            flags.are_boxes_visible = !flags.are_boxes_visible;
            world = draw_on_view(
                params.initial_view,
                flags.are_boxes_visible,
                world,
                shape_win,
            );
        }
        ReleasedKey::Delete => {
            // Remove selected
            let annos = get_annos_mut(&mut world);
            if !annos.selected_bbs().is_empty() {
                annos.remove_selected();
                world = draw_on_view(
                    params.initial_view,
                    flags.are_boxes_visible,
                    world,
                    shape_win,
                );
                history.push(Record::new(world.data.clone(), ACTOR_NAME));
            }
        }
        ReleasedKey::A if params.is_ctrl_held => {
            // Select all
            get_annos_mut(&mut world).select_all();
            world = draw_on_view(
                params.initial_view,
                flags.are_boxes_visible,
                world,
                shape_win,
            );
        }
        ReleasedKey::D if params.is_ctrl_held => {
            // Deselect all
            get_annos_mut(&mut world).deselect_all();
            world = draw_on_view(
                params.initial_view,
                flags.are_boxes_visible,
                world,
                shape_win,
            );
        }
        ReleasedKey::C if params.is_ctrl_held => {
            // Copy to clipboard
            if let Some(annos) = get_annos(&world) {
                get_tools_data_mut(&mut world)
                    .specifics
                    .bbox_mut()
                    .clipboard = Some(ClipboardData::from_annotations(annos));
                world = draw_on_view(
                    params.initial_view,
                    flags.are_boxes_visible,
                    world,
                    shape_win,
                );
            }
        }
        ReleasedKey::V if params.is_ctrl_held => {
            (world, history) = paste(params.initial_view, shape_win, world, history);
        }
        ReleasedKey::V => {
            flags.auto_paste = !flags.auto_paste;
        }
        ReleasedKey::L if params.is_ctrl_held => {
            let show_label = if let Some(annos) = get_annos(&world) {
                annos.show_labels
            } else {
                false
            };
            get_annos_mut(&mut world).show_labels = !show_label;
            world = draw_on_view(
                params.initial_view,
                flags.are_boxes_visible,
                world,
                shape_win,
            );
        }
        ReleasedKey::C => {
            // Paste selection directly at current mouse position
            let mp_orig =
                mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, world.zoom_box());
            if let Some((x_shift, y_shift)) = mp_orig {
                let shape_orig = world.shape_orig();
                let annos = get_annos_mut(&mut world);
                let selected_inds = selected_indices(annos.selected_bbs());
                let first_idx = selected_indices(annos.selected_bbs()).next();
                if let Some(first_idx) = first_idx {
                    let translated = selected_inds.flat_map(|idx| {
                        let bb = annos.bbs()[idx];
                        let first = annos.bbs()[first_idx];
                        bb.translate(
                            x_shift as i32 - first.x as i32,
                            y_shift as i32 - first.y as i32,
                            shape_orig,
                            OutOfBoundsMode::Deny,
                        )
                        .map(|bb| (bb, annos.cat_idxs()[idx]))
                    });
                    let translated_bbs = translated.clone().map(|(bb, _)| bb).collect::<Vec<_>>();
                    let translated_cat_ids =
                        translated.map(|(_, cat_id)| cat_id).collect::<Vec<_>>();

                    if !translated_bbs.is_empty() {
                        annos.extend(
                            translated_bbs.iter().copied(),
                            translated_cat_ids.iter().copied(),
                            shape_orig,
                        );
                        annos.deselect_all();
                        annos.select_last_n(translated_bbs.len());
                        world = draw_on_view(
                            params.initial_view,
                            flags.are_boxes_visible,
                            world,
                            shape_win,
                        );
                        history.push(Record::new(world.data.clone(), ACTOR_NAME));
                    }
                }
            }
        }
        ReleasedKey::Up | ReleasedKey::Down | ReleasedKey::Left | ReleasedKey::Right => {
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
        }
        _ => (),
    }
    get_tools_data_mut(&mut world).specifics.bbox_mut().options = flags;
    (world, history)
}

#[cfg(test)]
use {
    super::core::initialize_tools_menu_data,
    crate::{annotations::BboxAnnotations, domain::make_test_bbs, types::ViewImage},
    image::DynamicImage,
    std::collections::HashMap,
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
    let mut world = initialize_tools_menu_data(world);
    world.data.meta_data.is_loading_screen_active = Some(false);
    let tools_data = get_tools_data_mut(&mut world);
    tools_data
        .specifics
        .bbox_mut()
        .push("label".to_string(), None, None)
        .unwrap();
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
fn test_key_released() {
    let (initial_view, _, shape_win, mut world, history) = test_data();
    let make_params = |released_key, is_ctrl_held| KeyReleasedParams {
        initial_view: &initial_view,
        is_ctrl_held,
        released_key,
    };
    let annos = get_annos_mut(&mut world);
    annos.add_bb(
        BB {
            x: 1,
            y: 1,
            h: 10,
            w: 10,
        },
        0,
    );
    assert!(!annos.selected_bbs()[0]);
    let annos_orig = annos.clone();

    // select all boxes with ctrl+A
    let params = make_params(ReleasedKey::A, false);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    assert!(!get_annos(&world).unwrap().selected_bbs()[0]);
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    assert!(get_annos(&world).unwrap().selected_bbs()[0]);

    // copy and paste boxes to and from clipboard
    let params = make_params(ReleasedKey::C, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    assert!(get_annos(&world).unwrap().selected_bbs()[0]);
    if let Some(clipboard) = get_tools_data(&world).specifics.bbox().clipboard.clone() {
        let mut annos = BboxAnnotations::new();
        annos.extend(
            clipboard.bbs().iter().copied(),
            clipboard.cat_idxs().iter().copied(),
            Shape { w: 100, h: 100 },
        );
        assert_eq!(annos.bbs(), get_annos(&world).unwrap().bbs());
        assert_eq!(annos.cat_idxs(), get_annos(&world).unwrap().cat_idxs());
        assert_ne!(
            annos.selected_bbs(),
            get_annos(&world).unwrap().selected_bbs()
        );
    } else {
        assert!(false);
    }
    let params = make_params(ReleasedKey::V, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    assert!(get_tools_data(&world).specifics.bbox().clipboard.is_some());
    assert_eq!(get_annos(&world).unwrap().bbs(), annos_orig.bbs());
    let params = make_params(ReleasedKey::C, true);
    let (mut world, history) = on_key_released(world, history, None, shape_win, params);
    get_annos_mut(&mut world).remove(0);
    let params = make_params(ReleasedKey::V, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    assert_eq!(get_annos(&world).unwrap().bbs(), annos_orig.bbs());

    // clone box
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    let params = make_params(ReleasedKey::C, false);
    let (world, history) = on_key_released(world, history, Some((2, 2)), shape_win, params);
    assert_eq!(get_annos(&world).unwrap().bbs()[0], annos_orig.bbs()[0]);
    assert_eq!(
        get_annos(&world).unwrap().bbs()[1],
        annos_orig.bbs()[0]
            .translate(1, 1, world.shape_orig(), OutOfBoundsMode::Deny)
            .unwrap()
    );
    assert_eq!(get_annos(&world).unwrap().bbs().len(), 2);

    // deselect all boxes with ctrl+D
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    let params = make_params(ReleasedKey::D, false);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    assert!(get_annos(&world).unwrap().selected_bbs()[0]);
    let params = make_params(ReleasedKey::D, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    let flags = get_tools_data(&world).specifics.bbox().options;
    assert!(flags.are_boxes_visible);
    assert!(!get_annos(&world).unwrap().selected_bbs()[0]);

    // hide all boxes with ctrl+H
    let params = make_params(ReleasedKey::H, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    let flags = get_tools_data(&world).specifics.bbox().options;
    assert!(!flags.are_boxes_visible);

    // delete all selected boxes with ctrl+Delete
    let params = make_params(ReleasedKey::Delete, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    assert!(!get_annos(&world).unwrap().selected_bbs().is_empty());
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, shape_win, params);
    let params = make_params(ReleasedKey::Delete, true);
    let (world, _) = on_key_released(world, history, None, shape_win, params);
    assert!(get_annos(&world).unwrap().selected_bbs().is_empty());
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
        assert_eq!(get_annos(&world).unwrap().bbs()[0], bbs[0]);
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
        assert_ne!(get_annos(&world).unwrap().bbs()[0], bbs[0]);
        assert!(!history_equal(&history, &new_hist));
    }
}

#[test]
fn test_mouse_release() {
    let (initial_view, mouse_pos, shape_win, world, history) = test_data();
    let make_params = |prev_pos, is_ctrl_held| MouseReleaseParams {
        prev_pos: PrevPos {
            prev_pos,
            last_valid_click: prev_pos,
        },
        are_boxes_visible: true,
        is_alt_held: false,
        is_shift_held: false,
        is_ctrl_held,
        initial_view: &initial_view,
    };
    {
        // If a previous position was registered, we expect that the second click creates the
        // bounding box.
        let params = make_params(Some((30, 30)), false);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos.prev_pos, None);
        let annos = get_annos(&world);
        assert_eq!(annos.unwrap().bbs().len(), 1);
        assert_eq!(annos.unwrap().cat_idxs()[0], 0);
        assert!(format!("{:?}", new_hist).len() > format!("{:?}", history).len());
    }
    {
        // If no position was registered, a left click will trigger the start
        // of defining a new bounding box. The other corner will be defined by a second click.
        let params = make_params(None, false);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos.prev_pos, mouse_pos);
        let annos = get_annos(&world);
        assert!(annos.is_none() || annos.unwrap().bbs().is_empty());
        assert!(history_equal(&history, &new_hist));
    }
    {
        // If ctrl is hold, a bounding box would be selected. Since no bounding boxes exist,
        // nothing should happen.
        let params = make_params(None, true);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos.prev_pos, None);
        let annos = get_annos(&world);
        assert!(annos.is_none() || annos.unwrap().bbs().is_empty());
        assert!(history_equal(&history, &new_hist));
    }
    {
        // If ctrl is hold at the second click, this does not really make sense. We ignore it and assume this
        // is the finishing box click.
        let params = make_params(Some((30, 30)), true);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos.prev_pos, None);
        let annos = get_annos(&world);
        assert_eq!(annos.unwrap().bbs().len(), 1);
        assert!(!annos.unwrap().selected_bbs()[0]);
        assert!(format!("{:?}", new_hist).len() > format!("{:?}", history).len());
    }
    {
        // If ctrl is hold the box is selected.
        let params = make_params(None, true);
        let mut world = world.clone();
        get_tools_data_mut(&mut world)
            .specifics
            .bbox_mut()
            .push("label2".to_string(), None, None)
            .unwrap();
        get_tools_data_mut(&mut world)
            .specifics
            .bbox_mut()
            .cat_idx_current = 1;
        let annos = get_annos_mut(&mut world);
        annos.add_bb(BB::from_arr(&[20, 20, 20, 20]), 0);
        annos.add_bb(BB::from_arr(&[50, 50, 5, 5]), 0);
        annos.add_bb(BB::from_arr(&[20, 50, 3, 3]), 1);
        annos.add_bb(BB::from_arr(&[20, 55, 3, 3]), 0);

        let (mut world, _, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        let annos = get_annos(&world).unwrap();
        assert_eq!(prev_pos.prev_pos, None);
        assert!(annos.selected_bbs()[0]);
        assert!(!annos.selected_bbs()[1]);
        assert_eq!(annos.cat_idxs()[0], 0);
        assert_eq!(annos.cat_idxs()[1], 0);
        assert_eq!(annos.cat_idxs()[2], 1);
        assert_eq!(annos.cat_idxs()[3], 0);
        // alt
        get_tools_data_mut(&mut world)
            .specifics
            .bbox_mut()
            .cat_idx_current = 1;
        let mut params = make_params(None, true);
        params.is_alt_held = true;
        let annos = get_annos_mut(&mut world);
        annos.deselect_all();
        annos.select(1);
        let (mut world, _, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        let annos = get_annos(&world).unwrap();
        assert_eq!(prev_pos.prev_pos, None);
        assert!(annos.selected_bbs()[0]);
        assert!(!annos.selected_bbs()[1]);
        assert_eq!(annos.cat_idxs()[0], 1);
        assert_eq!(annos.cat_idxs()[1], 0);
        assert_eq!(annos.cat_idxs()[2], 1);
        assert_eq!(annos.cat_idxs()[3], 0);
        // shift
        let mut params = make_params(None, true);
        params.is_shift_held = true;
        let annos = get_annos_mut(&mut world);
        annos.select(3);
        let (world, _, prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world.clone(), history.clone());
        let annos = get_annos(&world).unwrap();
        assert_eq!(prev_pos.prev_pos, None);
        assert!(annos.selected_bbs()[0]);
        assert!(!annos.selected_bbs()[1]);
        assert!(annos.selected_bbs()[2]);
        assert!(annos.selected_bbs()[3]);
        assert_eq!(annos.cat_idxs()[0], 1);
        assert_eq!(annos.cat_idxs()[1], 0);
        assert_eq!(annos.cat_idxs()[2], 1);
        assert_eq!(annos.cat_idxs()[3], 0);
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
