use crate::{
    annotations::BboxAnnotations,
    annotations_accessor, annotations_accessor_mut,
    domain::{mouse_pos_to_orig_pos, Shape, BB},
    file_util,
    history::{History, Record},
    make_tool_transform,
    tools::{
        core::{InitialView, Mover},
        Manipulate,
    },
    tools_data::{BboxSpecificData, ToolSpecifics, ToolsData},
    tools_data_accessor, tools_data_accessor_mut, tools_data_initializer,
    world::World,
    LEFT_BTN, RIGHT_BTN,
};
use std::mem;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::on_events::{
    export_if_triggered, import_coco_if_triggered, map_released_key, on_key_released,
    on_mouse_held_right, on_mouse_released_left, KeyReleasedParams, MouseHeldParams,
    MouseReleaseParams,
};
pub const ACTOR_NAME: &str = "BBox";
const MISSING_ANNO_MSG: &str = "bbox annotations have not yet been initialized";
const MISSING_TOOLSMENU_MSG: &str = "bbox tools menu has not yet been initialized";
tools_data_initializer!(ACTOR_NAME, Bbox, BboxSpecificData);
tools_data_accessor!(ACTOR_NAME, MISSING_TOOLSMENU_MSG);
tools_data_accessor_mut!(ACTOR_NAME, MISSING_TOOLSMENU_MSG);
annotations_accessor_mut!(ACTOR_NAME, bbox_mut, MISSING_ANNO_MSG, BboxAnnotations);
annotations_accessor!(ACTOR_NAME, bbox, MISSING_ANNO_MSG, BboxAnnotations);

pub(super) fn paste(
    initial_view: &InitialView,
    shape_win: Shape,
    mut world: World,
    mut history: History,
) -> (World, History) {
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
            world = draw_on_view(initial_view, are_boxes_visible, world, shape_win);
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
        }
    }
    (world, history)
}

pub(super) fn current_cat_idx(world: &World) -> usize {
    get_tools_data(world).specifics.bbox().cat_idx_current
}

pub(super) fn draw_on_view(
    initial_view: &InitialView,
    are_boxes_visible: bool,
    mut world: World,
    shape_win: Shape,
) -> World {
    if are_boxes_visible {
        let bb_data = &get_tools_data(&world).specifics.bbox();
        if let Some(annos) = get_annos(&world) {
            let im_view = annos.draw_on_view(
                initial_view.image().clone().unwrap(),
                world.zoom_box(),
                world.data.shape(),
                shape_win,
                bb_data.labels(),
                bb_data.colors(),
            );
            world.set_im_view(im_view);
        }
    } else if let Some(iv) = initial_view.image() {
        world.set_im_view(iv.clone());
    }
    world
}

#[derive(Clone, Debug)]
pub struct BBox {
    prev_pos: Option<(usize, usize)>,
    initial_view: InitialView,
    mover: Mover,
    prev_label: usize,
    previous_file: Option<String>,
}

impl BBox {
    fn mouse_pressed(
        &mut self,
        _event: &WinitInputHelper,
        _shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        world: World,
        history: History,
    ) -> (World, History) {
        self.mover.move_mouse_pressed(mouse_pos);
        (world, history)
    }

    fn mouse_held(
        &mut self,
        _event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        world: World,
        history: History,
    ) -> (World, History) {
        let are_boxes_visible = get_tools_data(&world)
            .specifics
            .bbox()
            .flags
            .are_boxes_visible;
        let params = MouseHeldParams {
            are_boxes_visible,
            initial_view: &self.initial_view,
            mover: &mut self.mover,
        };
        on_mouse_held_right(shape_win, mouse_pos, params, world, history)
    }

    fn mouse_released(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if event.mouse_released(LEFT_BTN) {
            let are_boxes_visible = get_tools_data(&world)
                .specifics
                .bbox()
                .flags
                .are_boxes_visible;
            let params = MouseReleaseParams {
                prev_pos: self.prev_pos,
                are_boxes_visible,
                is_alt_held: event.held_alt(),
                is_shift_held: event.held_shift(),
                is_ctrl_held: event.held_control(),
                initial_view: &self.initial_view,
            };
            (world, history, self.prev_pos) =
                on_mouse_released_left(shape_win, mouse_pos, params, world, history);
        } else {
            history.push(Record::new(world.data.clone(), ACTOR_NAME))
        }
        (world, history)
    }

    fn key_held(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        let are_boxes_visible = get_tools_data(&world)
            .specifics
            .bbox()
            .flags
            .are_boxes_visible;
        // up, down, left, right
        let shape_orig = world.data.shape();
        let annos = get_annos_mut(&mut world);
        if event.key_held(VirtualKeyCode::Up) && event.held_control() {
            annos.shift_min_bbs(0, -1, shape_orig);
        } else if event.key_held(VirtualKeyCode::Down) && event.held_control() {
            annos.shift_min_bbs(0, 1, shape_orig);
        } else if event.key_held(VirtualKeyCode::Right) && event.held_control() {
            annos.shift_min_bbs(1, 0, shape_orig);
        } else if event.key_held(VirtualKeyCode::Left) && event.held_control() {
            annos.shift_min_bbs(-1, 0, shape_orig);
        } else if event.key_held(VirtualKeyCode::Up) && event.held_alt() {
            annos.shift(0, -1, shape_orig);
        } else if event.key_held(VirtualKeyCode::Down) && event.held_alt() {
            annos.shift(0, 1, shape_orig);
        } else if event.key_held(VirtualKeyCode::Right) && event.held_alt() {
            annos.shift(1, 0, shape_orig);
        } else if event.key_held(VirtualKeyCode::Left) && event.held_alt() {
            annos.shift(-1, 0, shape_orig);
        } else if event.key_held(VirtualKeyCode::Up) {
            annos.shift_max_bbs(0, -1, shape_orig);
        } else if event.key_held(VirtualKeyCode::Down) {
            annos.shift_max_bbs(0, 1, shape_orig);
        } else if event.key_held(VirtualKeyCode::Right) {
            annos.shift_max_bbs(1, 0, shape_orig);
        } else if event.key_held(VirtualKeyCode::Left) {
            annos.shift_max_bbs(-1, 0, shape_orig);
        }
        world = draw_on_view(&self.initial_view, are_boxes_visible, world, shape_win);
        world.update_view(shape_win);
        (world, history)
    }

    fn key_released(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let params = KeyReleasedParams {
            initial_view: &self.initial_view,
            is_ctrl_held: event.held_control(),
            released_key: map_released_key(event),
        };
        (world, history) = on_key_released(world, history, mouse_pos, shape_win, params);
        (world, history)
    }
}

impl Manipulate for BBox {
    fn new() -> Self {
        Self {
            prev_pos: None,
            initial_view: InitialView::new(),
            mover: Mover::new(),
            prev_label: 0,
            previous_file: None,
        }
    }

    fn on_activate(
        &mut self,
        mut world: World,
        mut history: History,
        shape_win: Shape,
    ) -> (World, History) {
        self.prev_pos = None;
        self.initial_view = InitialView::new();
        self.initial_view.update(&world, shape_win);
        world = initialize_tools_menu_data(world);
        get_tools_data_mut(&mut world).menu_active = true;
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
        (world, history)
    }

    fn on_deactivate(
        &mut self,
        mut world: World,
        history: History,
        _shape_win: Shape,
    ) -> (World, History) {
        self.prev_pos = None;
        self.initial_view = InitialView::new();
        get_tools_data_mut(&mut world).menu_active = false;
        (world, history)
    }

    fn events_tf(
        &mut self,
        mut world: World,
        mut history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &WinitInputHelper,
    ) -> (World, History) {
        if event.window_resized().is_some() {
            (world, history) = self.on_activate(world, history, shape_win);
        }
        let is_file_new = self.previous_file != world.data.meta_data.file_path;
        if is_file_new {
            {
                let bbox_data = get_tools_data_mut(&mut world).specifics.bbox_mut();
                for (_, (anno, _)) in bbox_data.anno_iter_mut() {
                    anno.deselect_all();
                }
            }
            self.previous_file = world.data.meta_data.file_path.clone();
        }
        let is_anno_rm_triggered = get_tools_data(&world)
            .specifics
            .bbox()
            .flags
            .is_anno_rm_triggered;
        if is_anno_rm_triggered {
            let opened_folder = world
                .data
                .meta_data
                .opened_folder
                .as_ref()
                .map(|of| file_util::url_encode(of));
            let bbox_data = get_tools_data_mut(&mut world).specifics.bbox_mut();
            if let Some(opened_folder) = &opened_folder {
                bbox_data.retain_fileannos_in_folder(opened_folder);
            }

            bbox_data.flags.is_anno_rm_triggered = false;
            world = draw_on_view(
                &self.initial_view,
                bbox_data.flags.are_boxes_visible,
                world,
                shape_win,
            );
        }
        // this is necessary in addition to the call in on_activate due to undo/redo
        world = initialize_tools_menu_data(world);
        {
            // export label file if demanded
            let bbox_data = get_tools_data(&world).specifics.bbox();
            export_if_triggered(&world.data.meta_data, bbox_data);
            get_tools_data_mut(&mut world)
                .specifics
                .bbox_mut()
                .export_trigger
                .is_export_triggered = false;
        }
        {
            // import coco if demanded
            let flags = get_tools_data(&world).specifics.bbox().flags;
            if let Some(imported_data) =
                import_coco_if_triggered(&world.data.meta_data, flags.is_coco_import_triggered)
            {
                *get_tools_data_mut(&mut world).specifics.bbox_mut() = imported_data;
                world = draw_on_view(
                    &self.initial_view,
                    flags.are_boxes_visible,
                    world,
                    shape_win,
                );
            } else {
                get_tools_data_mut(&mut world)
                    .specifics
                    .bbox_mut()
                    .flags
                    .is_coco_import_triggered = false;
            }
        }
        let in_menu_selected_label = current_cat_idx(&world);
        let flags = get_tools_data(&world).specifics.bbox().flags;
        if self.prev_label != in_menu_selected_label {
            world = draw_on_view(
                &self.initial_view,
                flags.are_boxes_visible,
                world,
                shape_win,
            );
        }
        self.initial_view.update(&world, shape_win);
        let bbox_data = get_tools_data_mut(&mut world).specifics.bbox_mut();
        if bbox_data.flags.auto_paste {
            (world, history) = paste(&self.initial_view, shape_win, world, history);
        }
        let mp_orig =
            mouse_pos_to_orig_pos(mouse_pos, world.data.shape(), shape_win, world.zoom_box());
        let pp_orig = mouse_pos_to_orig_pos(
            self.prev_pos,
            world.data.shape(),
            shape_win,
            world.zoom_box(),
        );
        if let (Some(mp), Some(pp)) = (mp_orig, pp_orig) {
            // animation
            world = draw_on_view(
                &self.initial_view,
                flags.are_boxes_visible,
                world,
                shape_win,
            );
            let tmp_annos =
                BboxAnnotations::from_bbs(vec![BB::from_points(mp, pp)], in_menu_selected_label);
            let mut im_view = world.take_view();
            let bb_data = get_tools_data(&world).specifics.bbox();
            im_view = tmp_annos.draw_on_view(
                im_view,
                world.zoom_box(),
                world.data.shape(),
                shape_win,
                bb_data.labels(),
                bb_data.colors(),
            );
            world.set_im_view(im_view);
        }
        (world, history) = make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            [
                (mouse_pressed, RIGHT_BTN),
                (mouse_held, RIGHT_BTN),
                (mouse_released, LEFT_BTN),
                (mouse_released, RIGHT_BTN)
            ],
            [
                (key_released, VirtualKeyCode::Delete),
                (key_released, VirtualKeyCode::H),
                (key_released, VirtualKeyCode::A),
                (key_released, VirtualKeyCode::D),
                (key_released, VirtualKeyCode::C),
                (key_released, VirtualKeyCode::V),
                (key_released, VirtualKeyCode::L),
                (key_released, VirtualKeyCode::Down),
                (key_released, VirtualKeyCode::Up),
                (key_released, VirtualKeyCode::Left),
                (key_released, VirtualKeyCode::Right),
                (key_held, VirtualKeyCode::Down),
                (key_held, VirtualKeyCode::Up),
                (key_held, VirtualKeyCode::Left),
                (key_held, VirtualKeyCode::Right)
            ]
        );
        self.prev_label = in_menu_selected_label;
        (world, history)
    }
}
