use crate::{
    annotations::SplitMode,
    cfg::CocoFile,
    domain::{shape_unscaled, OutOfBoundsMode, PtF, BB},
    events::{Events, KeyCode},
    file_util::MetaData,
    history::Record,
    tools::{core::Mover, BBOX_NAME},
    tools_data::{self, bbox_data::ClipboardData, BboxSpecificData},
    util::true_indices,
    {history::History, world::World},
};

use super::core::{
    current_cat_idx, get_annos, get_annos_mut, get_tools_data, get_tools_data_mut, paste,
    ACTOR_NAME,
};

const CORNER_TOL_DENOMINATOR: u32 = 5000;

fn find_closest_boundary_idx(pos: (f32, f32), bbs: &[BB]) -> Option<usize> {
    bbs.iter()
        .enumerate()
        .filter(|(_, bb)| bb.contains(pos))
        .map(|(i, bb)| {
            let dx = (bb.x as f32 - pos.0).abs();
            let dw = ((bb.x + bb.w) as f32 - pos.0).abs();
            let dy = (bb.y as f32 - pos.1).abs();
            let dh = ((bb.y + bb.h) as f32 - pos.1).abs();
            (i, dx.min(dw).min(dy).min(dh))
        })
        .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap())
        .map(|(i, _)| i)
}

/// returns index of the bounding box and the index of the closest close corner
fn find_close_corner(orig_pos: PtF, bbs: &[BB], tolerance: i64) -> Option<(usize, usize)> {
    let opi64: (i64, i64) = orig_pos.into();
    bbs.iter()
        .enumerate()
        .map(|(bb_idx, bb)| {
            let (min_corner_idx, min_corner_dist) = bb
                .corners()
                .map(|c| c.into())
                .map(|c: (i64, i64)| (opi64.0 - c.0).pow(2) + (opi64.1 - c.1).pow(2))
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
    coco_file: &CocoFile,
) -> Option<BboxSpecificData> {
    if is_coco_import_triggered {
        match tools_data::coco_io::read_coco(meta_data, coco_file) {
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
    pub mover: &'a mut Mover,
}
pub(super) fn on_mouse_held_right(
    mouse_pos: Option<PtF>,
    params: MouseHeldParams,
    mut world: World,
    mut history: History,
) -> (World, History) {
    let orig_shape = world.data.shape();
    let mut add_to_history = false;
    let move_boxes = |mpo_from, mpo_to| {
        let split_mode = get_tools_data(&world).specifics.bbox().options.split_mode;
        let annos = get_annos_mut(&mut world);
        add_to_history = annos.selected_follow_movement(mpo_from, mpo_to, orig_shape, split_mode);
        Some(())
    };
    params
        .mover
        .move_mouse_held(move_boxes, mouse_pos);
    if add_to_history {
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
    }
    let are_boxes_visible = get_tools_data(&world)
        .specifics
        .bbox()
        .options
        .are_boxes_visible;
    world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
    (world, history)
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PrevPos {
    pub prev_pos: Option<PtF>,
    pub last_valid_click: Option<PtF>,
}

pub(super) struct MouseReleaseParams {
    pub prev_pos: PrevPos,

    pub are_boxes_visible: bool,
    pub is_alt_held: bool,
    pub is_shift_held: bool,
    pub is_ctrl_held: bool,
}

pub(super) fn on_mouse_released_left(
    mouse_pos: Option<PtF>,
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
    } = params;
    let lc_orig = prev_pos.last_valid_click;
    let pp_orig = prev_pos.prev_pos;
    let in_menu_selected_label = current_cat_idx(&world);
    if let Some(mp) = mouse_pos {
        prev_pos.last_valid_click = Some(mp);
    }
    if let (Some(mp), Some(pp), Some(last_click)) = (mouse_pos, pp_orig, lc_orig) {
        // second click new bb
        if (mp.x as i32 - pp.x as i32).abs() > 1 && (mp.y as i32 - pp.y as i32).abs() > 1 {
            let mp = match split_mode {
                SplitMode::Horizontal => (last_click.x, mp.y).into(),
                SplitMode::Vertical => (mp.x, last_click.y).into(),
                SplitMode::None => mp,
            };
            let annos = get_annos_mut(&mut world);
            annos.add_bb(
                BB::from_points(mp.into(), pp.into()),
                in_menu_selected_label,
            );
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
            prev_pos.prev_pos = None;
            world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        }
    } else if is_ctrl_held || is_alt_held || is_shift_held {
        // selection
        let annos = get_annos_mut(&mut world);
        let idx = mouse_pos.and_then(|p| find_closest_boundary_idx((p.x, p.y), annos.bbs()));
        if let Some(i) = idx {
            if is_shift_held {
                // If shift is held a new selection box will be spanned between the currently clicked
                // box and the selected box that has the maximum distance in terms of max-corner-dist.
                // All boxes that have overlap with this new selection box will be selected. If no box
                // is selected only the currently clicked box will be selected.
                annos.select(i);
                let newly_selected_bb = &annos.bbs()[i];
                let sel_indxs = true_indices(annos.selected_bbs());
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
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
    } else {
        let shape_orig = world.data.shape();
        let unscaled = shape_unscaled(world.zoom_box(), shape_orig);
        let tolerance = (unscaled.w * unscaled.h / CORNER_TOL_DENOMINATOR).max(2);
        let close_corner = mouse_pos.and_then(|mp| {
            get_annos(&world).and_then(|a| find_close_corner(mp, a.bbs(), tolerance as i64))
        });
        if let Some((bb_idx, idx)) = close_corner {
            // move an existing corner
            let annos = get_annos_mut(&mut world);
            let bb = annos.remove(bb_idx);
            let oppo_corner = bb.opposite_corner(idx);
            prev_pos.prev_pos = Some(oppo_corner.into());
        } else {
            match split_mode {
                SplitMode::None => {
                    // first click new bb
                    prev_pos.prev_pos = mouse_pos;
                }
                _ => {
                    // create boxes by splitting either horizontally or vertically
                    if let Some(mp) = mouse_pos {
                        let existing_bbs: &[BB] = if let Some(annos) = get_annos(&world) {
                            annos.bbs()
                        } else {
                            &[]
                        };
                        let new_bbs = if let SplitMode::Horizontal = split_mode {
                            if let Some((i, bb)) = existing_bbs
                                .iter()
                                .enumerate()
                                .find(|(_, bb)| bb.contains((mp.x, mp.y)))
                            {
                                let (top, btm) = bb.split_horizontally(mp.y as u32);
                                vec![(Some(i), top, btm)]
                            } else {
                                let new_bbs = existing_bbs
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, bb)| bb.covers_y(mp.y as u32))
                                    .map(|(i, bb)| {
                                        let (top, btm) = bb.split_horizontally(mp.y as u32);
                                        (Some(i), top, btm)
                                    })
                                    .collect::<Vec<_>>();
                                if new_bbs.is_empty() {
                                    let (top, btm) =
                                        BB::from_shape(shape_orig).split_horizontally(mp.y as u32);
                                    vec![(None, top, btm)]
                                } else {
                                    new_bbs
                                }
                            }
                        // SplitMode::Vertical
                        } else if let Some((i, bb)) = existing_bbs
                            .iter()
                            .enumerate()
                            .find(|(_, bb)| bb.contains((mp.x, mp.y)))
                        {
                            let (left, right) = bb.split_vertically(mp.x as u32);
                            vec![(Some(i), left, right)]
                        } else {
                            let new_bbs = existing_bbs
                                .iter()
                                .enumerate()
                                .filter(|(_, bb)| bb.covers_x(mp.x as u32))
                                .map(|(i, bb)| {
                                    let (left, right) = bb.split_vertically(mp.x as u32);
                                    (Some(i), left, right)
                                })
                                .collect::<Vec<_>>();
                            if new_bbs.is_empty() {
                                let (left, right) =
                                    BB::from_shape(shape_orig).split_vertically(mp.x as u32);
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
                        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
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
        pub(super) fn map_released_key(event: &Events) -> ReleasedKey {
            if false {
                ReleasedKey::None
            } $(else if event.released(KeyCode::$key) {
                ReleasedKey::$key
            })*
            else {
                ReleasedKey::None
            }
        }
    };
}

released_key!(
    A, D, H, C, V, L, Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9, Delete, Back,
    Left, Right, Up, Down
);

macro_rules! set_cat_current {
    ($num:expr, $world:expr) => {
        let specifics = get_tools_data_mut(&mut $world).specifics.bbox_mut();
        if $num < specifics.cat_ids().len() + 1 {
            specifics.cat_idx_current = $num - 1;
        }
    };
}

pub(super) struct KeyReleasedParams {
    pub is_ctrl_held: bool,
    pub released_key: ReleasedKey,
}

pub(super) fn on_key_released(
    mut world: World,
    mut history: History,
    mouse_pos: Option<PtF>,
    params: KeyReleasedParams,
) -> (World, History) {
    let mut flags = get_tools_data_mut(&mut world).specifics.bbox_mut().options;
    match params.released_key {
        ReleasedKey::H if params.is_ctrl_held => {
            // Hide all boxes (selected or not)
            flags.are_boxes_visible = !flags.are_boxes_visible;
            world.request_redraw_annotations(BBOX_NAME, flags.are_boxes_visible);
        }
        ReleasedKey::Delete | ReleasedKey::Back => {
            // Remove selected
            let annos = get_annos_mut(&mut world);
            if !annos.selected_bbs().is_empty() {
                annos.remove_selected();
                world.request_redraw_annotations(BBOX_NAME, flags.are_boxes_visible);
                history.push(Record::new(world.data.clone(), ACTOR_NAME));
            }
        }
        ReleasedKey::A if params.is_ctrl_held => {
            // Select all
            get_annos_mut(&mut world).select_all();
            world.request_redraw_annotations(BBOX_NAME, flags.are_boxes_visible);
        }
        ReleasedKey::D if params.is_ctrl_held => {
            // Deselect all
            get_annos_mut(&mut world).deselect_all();
            world.request_redraw_annotations(BBOX_NAME, flags.are_boxes_visible);
        }
        ReleasedKey::C if params.is_ctrl_held => {
            // Copy to clipboard
            if let Some(annos) = get_annos(&world) {
                get_tools_data_mut(&mut world)
                    .specifics
                    .bbox_mut()
                    .clipboard = Some(ClipboardData::from_annotations(annos));
                world.request_redraw_annotations(BBOX_NAME, flags.are_boxes_visible);
            }
        }
        ReleasedKey::V if params.is_ctrl_held => {
            (world, history) = paste(world, history);
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
            world.request_redraw_annotations(BBOX_NAME, flags.are_boxes_visible);
        }
        ReleasedKey::C => {
            // Paste selection directly at current mouse position
            if let Some((x_shift, y_shift)) =
                mouse_pos.map(|mp| <PtF as Into<(i32, i32)>>::into(mp))
            {
                let shape_orig = world.shape_orig();
                let annos = get_annos_mut(&mut world);
                let selected_inds = true_indices(annos.selected_bbs());
                let first_idx = true_indices(annos.selected_bbs()).next();
                if let Some(first_idx) = first_idx {
                    let translated = selected_inds.flat_map(|idx| {
                        let bb = annos.bbs()[idx];
                        let first = annos.bbs()[first_idx];
                        bb.translate(
                            x_shift - first.x as i32,
                            y_shift - first.y as i32,
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
                        world.request_redraw_annotations(BBOX_NAME, flags.are_boxes_visible);
                        history.push(Record::new(world.data.clone(), ACTOR_NAME));
                    }
                }
            }
        }
        ReleasedKey::Up | ReleasedKey::Down | ReleasedKey::Left | ReleasedKey::Right => {
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
        }
        ReleasedKey::Key1 => {
            set_cat_current!(1, world);
        }
        ReleasedKey::Key2 => {
            set_cat_current!(2, world);
        }
        ReleasedKey::Key3 => {
            set_cat_current!(3, world);
        }
        ReleasedKey::Key4 => {
            set_cat_current!(4, world);
        }
        ReleasedKey::Key5 => {
            set_cat_current!(5, world);
        }
        ReleasedKey::Key6 => {
            set_cat_current!(6, world);
        }
        ReleasedKey::Key7 => {
            set_cat_current!(7, world);
        }
        ReleasedKey::Key8 => {
            set_cat_current!(8, world);
        }
        ReleasedKey::Key9 => {
            set_cat_current!(9, world);
        }
        _ => (),
    }
    get_tools_data_mut(&mut world).specifics.bbox_mut().options = flags;
    (world, history)
}

#[cfg(test)]
use {
    super::core::initialize_tools_menu_data,
    crate::{
        annotations::BboxAnnotations,
        domain::{make_test_bbs, Shape},
        point,
        types::ViewImage,
    },
    image::DynamicImage,
    std::collections::HashMap,
};

#[cfg(test)]
fn test_data() -> (Option<PtF>, World, History) {
    let im_test = DynamicImage::ImageRgb8(ViewImage::new(64, 64));
    let world = World::from_real_im(im_test, HashMap::new(), "superimage.png".to_string());
    let mut world = initialize_tools_menu_data(world);
    world.data.meta_data.is_loading_screen_active = Some(false);
    let tools_data = get_tools_data_mut(&mut world);
    tools_data
        .specifics
        .bbox_mut()
        .push("label".to_string(), None, None)
        .unwrap();
    let history = History::default();
    let mouse_pos = Some(point!(32.0, 32.0));
    (mouse_pos, world, history)
}

#[cfg(test)]
fn history_equal(hist1: &History, hist2: &History) -> bool {
    format!("{:?}", hist1) == format!("{:?}", hist2)
}

#[test]
fn test_key_released() {
    let (_, mut world, history) = test_data();
    let make_params = |released_key, is_ctrl_held| KeyReleasedParams {
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
    let (world, history) = on_key_released(world, history, None, params);
    assert!(!get_annos(&world).unwrap().selected_bbs()[0]);
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(get_annos(&world).unwrap().selected_bbs()[0]);

    // copy and paste boxes to and from clipboard
    let params = make_params(ReleasedKey::C, true);
    let (world, history) = on_key_released(world, history, None, params);
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
    let (world, history) = on_key_released(world, history, None, params);
    assert!(get_tools_data(&world).specifics.bbox().clipboard.is_some());
    assert_eq!(get_annos(&world).unwrap().bbs(), annos_orig.bbs());
    let params = make_params(ReleasedKey::C, true);
    let (mut world, history) = on_key_released(world, history, None, params);
    get_annos_mut(&mut world).remove(0);
    let params = make_params(ReleasedKey::V, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert_eq!(get_annos(&world).unwrap().bbs(), annos_orig.bbs());

    // clone box
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, params);
    let params = make_params(ReleasedKey::C, false);
    let (world, history) = on_key_released(world, history, Some(point!(2.0, 2.0)), params);
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
    let (world, history) = on_key_released(world, history, None, params);
    let params = make_params(ReleasedKey::D, false);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(get_annos(&world).unwrap().selected_bbs()[0]);
    let params = make_params(ReleasedKey::D, true);
    let (world, history) = on_key_released(world, history, None, params);
    let flags = get_tools_data(&world).specifics.bbox().options;
    assert!(flags.are_boxes_visible);
    assert!(!get_annos(&world).unwrap().selected_bbs()[0]);

    // hide all boxes with ctrl+H
    let params = make_params(ReleasedKey::H, true);
    let (world, history) = on_key_released(world, history, None, params);
    let flags = get_tools_data(&world).specifics.bbox().options;
    assert!(!flags.are_boxes_visible);

    // delete all selected boxes with ctrl+Delete
    let params = make_params(ReleasedKey::Delete, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(!get_annos(&world).unwrap().selected_bbs().is_empty());
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, params);
    let params = make_params(ReleasedKey::Delete, true);
    let (world, _) = on_key_released(world, history, None, params);
    assert!(get_annos(&world).unwrap().selected_bbs().is_empty());
}

#[test]
fn test_mouse_held() {
    let (mouse_pos, mut world, history) = test_data();
    let annos = get_annos_mut(&mut world);
    let bbs = make_test_bbs();
    annos.add_bb(bbs[0].clone(), 0);
    {
        let mut mover = Mover::new();
        mover.move_mouse_pressed(Some(point!(12.0, 12.0)));
        let params = MouseHeldParams {
            mover: &mut mover,
        };
        let (world, new_hist) =
            on_mouse_held_right(mouse_pos, params, world.clone(), history.clone());
        assert_eq!(get_annos(&world).unwrap().bbs()[0], bbs[0]);
        assert!(history_equal(&history, &new_hist));
    }
    {
        let mut mover = Mover::new();
        mover.move_mouse_pressed(Some(point!(12.0, 12.0)));
        let params = MouseHeldParams {
            mover: &mut mover,
        };
        let annos = get_annos_mut(&mut world);
        annos.select(0);
        let (world, new_hist) = on_mouse_held_right(mouse_pos, params, world, history.clone());
        assert_ne!(get_annos(&world).unwrap().bbs()[0], bbs[0]);

        println!("{:?}", bbs[0]);
        println!("{:?}", get_annos(&world).unwrap().bbs()[0]);
        assert!(!history_equal(&history, &new_hist));
    }
}

#[test]
fn test_mouse_release() {
    let (mouse_pos, world, history) = test_data();
    let make_params = |prev_pos, is_ctrl_held| MouseReleaseParams {
        prev_pos: PrevPos {
            prev_pos,
            last_valid_click: prev_pos,
        },
        are_boxes_visible: true,
        is_alt_held: false,
        is_shift_held: false,
        is_ctrl_held,
    };
    {
        // If a previous position was registered, we expect that the second click creates the
        // bounding box.
        let params = make_params(Some(point!(30.0, 30.0)), false);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
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
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
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
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos.prev_pos, None);
        let annos = get_annos(&world);
        assert!(annos.is_none() || annos.unwrap().bbs().is_empty());
        assert!(history_equal(&history, &new_hist));
    }
    {
        // If ctrl is hold at the second click, this does not really make sense. We ignore it and assume this
        // is the finishing box click.
        let params = make_params(Some(point!(30.0, 30.0)), true);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
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
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
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
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
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
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
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
    assert_eq!(find_closest_boundary_idx((0.0, 20.0), &bbs), None);
    assert_eq!(find_closest_boundary_idx((0.0, 0.0), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((3.0, 8.0), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((7.0, 14.0), &bbs), Some(1));
    assert_eq!(find_closest_boundary_idx((7.0, 15.0), &bbs), None);
    assert_eq!(find_closest_boundary_idx((8.0, 8.0), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((10.0, 12.0), &bbs), Some(2));
}
