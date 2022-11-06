use crate::tools::{
    core::{InitialView, Mover},
    Manipulate,
};
use crate::{
    annotations::BboxAnnotations,
    history::{History, Record},
    make_tool_transform,
    util::{self, mouse_pos_to_orig_pos, Shape, BB},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use self::{
    core::{
        current_cat_id, draw_on_view, get_annos_mut, get_tools_data, get_tools_data_mut,
        initialize_tools_menu_data, ACTOR_NAME,
    },
    on_events::{
        export_if_triggered, on_mouse_held_right, on_mouse_released_left, MouseHeldParams,
        MouseReleaseParams,
    },
};
mod core;
mod io;
mod on_events;

#[derive(Clone, Debug)]
pub struct BBox {
    prev_pos: Option<(usize, usize)>,
    initial_view: InitialView,
    mover: Mover,
    prev_label: usize,
    are_boxes_visible: bool,
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
        let params = MouseHeldParams {
            are_boxes_visible: self.are_boxes_visible,
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
        let params = MouseReleaseParams {
            prev_pos: self.prev_pos,
            are_boxes_visible: self.are_boxes_visible,
            is_ctrl_held: event.held_control(),
            initial_view: &self.initial_view,
        };
        (world, history, self.prev_pos) =
            on_mouse_released_left(shape_win, mouse_pos, params, world, history);
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
        // up, down, left, right
        let shape_orig = world.data.shape();
        let annos = get_annos_mut(&mut world);
        if util::with_control(VirtualKeyCode::Up, |x| event.key_held(x)) {
            annos.resize_bbs(0, -1, shape_orig);
        } else if util::with_control(VirtualKeyCode::Down, |x| event.key_held(x)) {
            annos.resize_bbs(0, 1, shape_orig);
        } else if util::with_control(VirtualKeyCode::Right, |x| event.key_held(x)) {
            annos.resize_bbs(1, 0, shape_orig);
        } else if util::with_control(VirtualKeyCode::Left, |x| event.key_held(x)) {
            annos.resize_bbs(-1, 0, shape_orig);
        }
        world = draw_on_view(&self.initial_view, self.are_boxes_visible, world, shape_win);
        world.update_view(shape_win);
        (world, history)
    }
    fn key_released(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if event.key_released(VirtualKeyCode::H) && event.held_control() {
            self.are_boxes_visible = !self.are_boxes_visible;
            world = draw_on_view(&self.initial_view, self.are_boxes_visible, world, shape_win);
        } else if event.key_released(VirtualKeyCode::Delete) {
            let annos = get_annos_mut(&mut world);
            annos.remove_selected();
            world = draw_on_view(&self.initial_view, self.are_boxes_visible, world, shape_win);
            world.update_view(shape_win);
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
        }
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
            are_boxes_visible: true,
        }
    }

    fn on_activate(
        &mut self,
        mut world: World,
        history: History,
        _shape_win: Shape,
    ) -> (World, History) {
        self.prev_pos = None;
        self.initial_view = InitialView::new();
        world = initialize_tools_menu_data(world);
        get_tools_data_mut(&mut world).menu_active = true;
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

        let bbox_data = get_tools_data(&world).specifics.bbox();
        let write_label_file = export_if_triggered(&world.data.meta_data, bbox_data.clone());
        get_tools_data_mut(&mut world)
            .specifics
            .bbox_mut()
            .write_label_file = write_label_file;

        let in_menu_selected_label = current_cat_id(&world);
        if self.prev_label != in_menu_selected_label {
            world = draw_on_view(&self.initial_view, self.are_boxes_visible, world, shape_win);
        }
        self.initial_view.update(&world, shape_win);
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
            world = draw_on_view(&self.initial_view, self.are_boxes_visible, world, shape_win);
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
                (mouse_released, LEFT_BTN)
            ],
            [
                (key_released, VirtualKeyCode::Delete),
                (key_released, VirtualKeyCode::H),
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
