use crate::{
    annotations::BboxAnnotations,
    annotations_accessor, annotations_accessor_mut,
    domain::{Shape, BB},
    drawme::{Annotation, Stroke},
    events::{Events, KeyCode},
    file_util,
    history::{History, Record},
    make_tool_transform,
    tools::{core::Mover, Manipulate, BBOX_NAME},
    tools_data::{bbox_data::Options, BboxSpecificData, ToolSpecifics, ToolsData},
    tools_data_accessor, tools_data_accessor_mut, tools_data_initializer,
    world::World,
    GeoFig,
};
use std::mem;

use super::on_events::{
    export_if_triggered, import_coco_if_triggered, map_released_key, on_key_released,
    on_mouse_held_right, on_mouse_released_left, KeyReleasedParams, MouseHeldParams,
    MouseReleaseParams, PrevPos,
};
pub const ACTOR_NAME: &str = "BBox";
const MISSING_ANNO_MSG: &str = "bbox annotations have not yet been initialized";
const MISSING_TOOLSMENU_MSG: &str = "bbox tools menu has not yet been initialized";
tools_data_initializer!(ACTOR_NAME, Bbox, BboxSpecificData);
tools_data_accessor!(ACTOR_NAME, MISSING_TOOLSMENU_MSG);
tools_data_accessor_mut!(ACTOR_NAME, MISSING_TOOLSMENU_MSG);
annotations_accessor_mut!(ACTOR_NAME, bbox_mut, MISSING_ANNO_MSG, BboxAnnotations);
annotations_accessor!(ACTOR_NAME, bbox, MISSING_ANNO_MSG, BboxAnnotations);

fn are_boxes_visible(world: &World) -> bool {
    get_tools_data(world)
        .specifics
        .bbox()
        .options
        .are_boxes_visible
}

pub(super) fn paste(mut world: World, mut history: History) -> (World, History) {
    // Paste from clipboard
    if let Some(clipboard) = mem::take(
        &mut get_tools_data_mut(&mut world)
            .specifics
            .bbox_mut()
            .clipboard,
    ) {
        let cb_bbs = clipboard.bbs();
        if !cb_bbs.is_empty() {
            let shape_orig = Shape::from_im(world.data.im_background());
            get_annos_mut(&mut world).extend(
                cb_bbs.iter().copied(),
                clipboard.cat_idxs().iter().copied(),
                shape_orig,
            );
            get_tools_data_mut(&mut world)
                .specifics
                .bbox_mut()
                .clipboard = Some(clipboard);
            let are_boxes_visible = true;
            get_tools_data_mut(&mut world)
                .specifics
                .bbox_mut()
                .options
                .are_boxes_visible = are_boxes_visible;
            world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
        }
    }
    (world, history)
}

pub(super) fn current_cat_idx(world: &World) -> usize {
    get_tools_data(world).specifics.bbox().cat_idx_current
}

fn check_recolorboxes(mut world: World) -> World {
    // check if re-color was triggered
    let options = get_tools_data(&world).specifics.bbox().options;
    if options.is_colorchange_triggered {
        // we show annotations after recoloring
        let are_boxes_visible = true;
        {
            let data = get_tools_data_mut(&mut world).specifics.bbox_mut();
            data.new_random_colors();
            data.options.is_colorchange_triggered = false;
            data.options.are_boxes_visible = true;
        }
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
    }
    world
}

fn check_filechange(
    mut world: World,
    previous_file: Option<String>,
) -> (bool, World, Option<String>) {
    let is_file_new = previous_file != world.data.meta_data.file_path;
    if is_file_new {
        {
            let bbox_data = get_tools_data_mut(&mut world).specifics.bbox_mut();
            for (_, (anno, _)) in bbox_data.anno_iter_mut() {
                anno.deselect_all();
            }
            let are_boxes_visible = bbox_data.options.are_boxes_visible;
            world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        }
        let new_file_path = world.data.meta_data.file_path.clone();
        (is_file_new, world, new_file_path)
    } else {
        (is_file_new, world, previous_file)
    }
}

fn check_annoremove(mut world: World) -> World {
    let is_anno_rm_triggered = get_tools_data(&world)
        .specifics
        .bbox()
        .options
        .is_anno_rm_triggered;
    if is_anno_rm_triggered {
        let opened_folder = world
            .data
            .meta_data
            .opened_folder
            .as_ref()
            .map(|of| file_util::url_encode(of));

        // we show annotations after recoloring
        let are_boxes_visible = true;
        {
            let data = get_tools_data_mut(&mut world).specifics.bbox_mut();
            if let Some(opened_folder) = &opened_folder {
                data.retain_fileannos_in_folder(opened_folder);
            }

            data.options.is_anno_rm_triggered = false;
            data.options.are_boxes_visible = are_boxes_visible;
        }
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
    }
    world
}

fn check_cocoexport(mut world: World) -> World {
    // export label file if demanded
    let bbox_data = get_tools_data(&world).specifics.bbox();
    export_if_triggered(&world.data.meta_data, bbox_data);
    get_tools_data_mut(&mut world)
        .specifics
        .bbox_mut()
        .options
        .is_export_triggered = false;
    world
}

fn check_cocoimport(mut world: World) -> World {
    // import coco if demanded
    let flags = get_tools_data(&world).specifics.bbox().options;
    if let Some(imported_data) = import_coco_if_triggered(
        &world.data.meta_data,
        flags.is_coco_import_triggered,
        &get_tools_data(&world).specifics.bbox().coco_file,
    ) {
        let are_boxes_visible = imported_data.options.are_boxes_visible;
        *get_tools_data_mut(&mut world).specifics.bbox_mut() = imported_data;
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
    } else {
        get_tools_data_mut(&mut world)
            .specifics
            .bbox_mut()
            .options
            .is_coco_import_triggered = false;
    }
    world
}

fn check_labelchange(mut world: World, prev_label: usize, options: Options) -> World {
    let in_menu_selected_label = current_cat_idx(&world);
    if prev_label != in_menu_selected_label {
        world.request_redraw_annotations(BBOX_NAME, options.are_boxes_visible);
    }
    world
}

fn check_autopaste(
    mut world: World,
    mut history: History,
    auto_paste: bool,
    is_file_changed: bool,
) -> (World, History) {
    if world.data.meta_data.is_loading_screen_active == Some(false) && is_file_changed && auto_paste
    {
        (world, history) = paste(world, history);
    }
    (world, history)
}

#[derive(Clone, Debug)]
pub struct BBox {
    prev_pos: PrevPos,
    mover: Mover,
    prev_label: usize,
    previous_file: Option<String>,
}

impl BBox {
    fn mouse_pressed(
        &mut self,
        event: &Events,
        world: World,
        history: History,
    ) -> (World, History) {
        self.mover.move_mouse_pressed(event.mouse_pos);
        (world, history)
    }

    fn mouse_held(&mut self, event: &Events, world: World, history: History) -> (World, History) {
        let params = MouseHeldParams {
            mover: &mut self.mover,
        };
        on_mouse_held_right(event.mouse_pos, params, world, history)
    }

    fn mouse_released(
        &mut self,
        event: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if event.released(KeyCode::MouseLeft) {
            let are_boxes_visible = are_boxes_visible(&world);
            let params = MouseReleaseParams {
                prev_pos: self.prev_pos,
                are_boxes_visible,
                is_alt_held: event.held_alt(),
                is_shift_held: event.held_shift(),
                is_ctrl_held: event.held_ctrl(),
            };
            (world, history, self.prev_pos) =
                on_mouse_released_left(event.mouse_pos, params, world, history);
        } else {
            history.push(Record::new(world.data.clone(), ACTOR_NAME))
        }
        (world, history)
    }

    fn key_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        let are_boxes_visible = get_tools_data(&world)
            .specifics
            .bbox()
            .options
            .are_boxes_visible;
        // up, down, left, right
        let shape_orig = world.data.shape();
        let split_mode = get_tools_data(&world).specifics.bbox().options.split_mode;
        let annos = get_annos_mut(&mut world);
        if events.held(KeyCode::Up) && events.held_ctrl() {
            annos.shift_min_bbs(0, -1, shape_orig, split_mode);
        } else if events.held(KeyCode::Down) && events.held_ctrl() {
            annos.shift_min_bbs(0, 1, shape_orig, split_mode);
        } else if events.held(KeyCode::Right) && events.held_ctrl() {
            annos.shift_min_bbs(1, 0, shape_orig, split_mode);
        } else if events.held(KeyCode::Left) && events.held_ctrl() {
            annos.shift_min_bbs(-1, 0, shape_orig, split_mode);
        } else if events.held(KeyCode::Up) && events.held_alt() {
            annos.shift(0, -1, shape_orig, split_mode);
        } else if events.held(KeyCode::Down) && events.held_alt() {
            annos.shift(0, 1, shape_orig, split_mode);
        } else if events.held(KeyCode::Right) && events.held_alt() {
            annos.shift(1, 0, shape_orig, split_mode);
        } else if events.held(KeyCode::Left) && events.held_alt() {
            annos.shift(-1, 0, shape_orig, split_mode);
        } else if events.held(KeyCode::Up) {
            annos.shift_max_bbs(0, -1, shape_orig, split_mode);
        } else if events.held(KeyCode::Down) {
            annos.shift_max_bbs(0, 1, shape_orig, split_mode);
        } else if events.held(KeyCode::Right) {
            annos.shift_max_bbs(1, 0, shape_orig, split_mode);
        } else if events.held(KeyCode::Left) {
            annos.shift_max_bbs(-1, 0, shape_orig, split_mode);
        }
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        (world, history)
    }

    fn key_released(
        &mut self,
        events: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let params = KeyReleasedParams {
            is_ctrl_held: events.held_ctrl(),
            released_key: map_released_key(events),
        };
        (world, history) = on_key_released(world, history, events.mouse_pos, params);
        (world, history)
    }
}

impl Manipulate for BBox {
    fn new() -> Self {
        Self {
            prev_pos: PrevPos::default(),
            mover: Mover::new(),
            prev_label: 0,
            previous_file: None,
        }
    }

    fn on_activate(&mut self, mut world: World, mut history: History) -> (World, History) {
        self.prev_pos = PrevPos::default();
        world = initialize_tools_menu_data(world);
        get_tools_data_mut(&mut world).menu_active = true;
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
        let are_boxes_visible = true;
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        (world, history)
    }

    fn on_deactivate(&mut self, mut world: World, history: History) -> (World, History) {
        self.prev_pos = PrevPos::default();
        get_tools_data_mut(&mut world).menu_active = false;
        let are_boxes_visible = false;
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        (world, history)
    }

    fn events_tf(
        &mut self,
        mut world: World,
        mut history: History,
        events: &Events,
    ) -> (World, History) {
        world = check_recolorboxes(world);
        let is_file_changed;
        (is_file_changed, world, self.previous_file) =
            check_filechange(world, mem::take(&mut self.previous_file));

        world = check_annoremove(world);

        // this is necessary in addition to the call in on_activate due to undo/redo
        world = initialize_tools_menu_data(world);

        world = check_cocoexport(world);

        world = check_cocoimport(world);

        let options = get_tools_data(&world).specifics.bbox().options;

        world = check_labelchange(world, self.prev_label, options);

        (world, history) = check_autopaste(world, history, options.auto_paste, is_file_changed);

        let in_menu_selected_label = current_cat_idx(&world);
        if let (Some(mp), Some(pp)) = (events.mouse_pos, self.prev_pos.prev_pos) {
            // animation
            let bb_data = get_tools_data(&world).specifics.bbox();
            let label = Some(bb_data.labels()[in_menu_selected_label].clone());
            let color = bb_data.colors()[in_menu_selected_label];
            let anno = Annotation {
                geofig: GeoFig::BB(BB::from_points(mp.into(), pp.into())),
                label,
                fill_color: Some(color),
                outline: Stroke::from_color(color),
                is_selected: None,
            };
            let are_boxes_visible = are_boxes_visible(&world);
            world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
            world.request_redraw_tmp_anno(anno);
        }
        (world, history) = make_tool_transform!(
            self,
            world,
            history,
            events,
            [
                (pressed, KeyCode::MouseRight, mouse_pressed),
                (held, KeyCode::MouseRight, mouse_held),
                (released, KeyCode::MouseLeft, mouse_released),
                (released, KeyCode::MouseRight, mouse_released),
                (released, KeyCode::Delete, key_released),
                (released, KeyCode::Back, key_released),
                (released, KeyCode::H, key_released),
                (released, KeyCode::A, key_released),
                (released, KeyCode::D, key_released),
                (released, KeyCode::C, key_released),
                (released, KeyCode::V, key_released),
                (released, KeyCode::L, key_released),
                (released, KeyCode::Down, key_released),
                (released, KeyCode::Up, key_released),
                (released, KeyCode::Left, key_released),
                (released, KeyCode::Right, key_released),
                (released, KeyCode::Key1, key_released),
                (released, KeyCode::Key2, key_released),
                (released, KeyCode::Key3, key_released),
                (released, KeyCode::Key4, key_released),
                (released, KeyCode::Key5, key_released),
                (released, KeyCode::Key6, key_released),
                (released, KeyCode::Key7, key_released),
                (released, KeyCode::Key8, key_released),
                (released, KeyCode::Key9, key_released),
                (held, KeyCode::Down, key_held),
                (held, KeyCode::Up, key_held),
                (held, KeyCode::Left, key_held),
                (held, KeyCode::Right, key_held)
            ]
        );
        self.prev_label = in_menu_selected_label;
        (world, history)
    }
}
