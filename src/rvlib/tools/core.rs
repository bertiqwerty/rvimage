use crate::domain::Annotate;
use crate::history::Record;
use crate::result::RvResult;
use crate::tools_data::annotations::{ClipboardData, InstanceAnnotations};
use crate::tools_data::{get_mut, get_specific_mut, CoreOptions, LabelInfo, ToolSpecifics};
use crate::ShapeI;
use crate::{domain::PtF, events::Events, history::History, world::World};

pub(super) fn check_trigger_redraw(
    mut world: World,
    name: &'static str,
    f_tool_access: impl FnMut(&mut ToolSpecifics) -> RvResult<&mut CoreOptions> + Clone,
) -> World {
    let data_mut = get_mut(&mut world, name, "could not access data");
    let core_options = get_specific_mut(f_tool_access.clone(), data_mut).cloned();
    if core_options.map(|o| o.is_redraw_annos_triggered) == Some(true) {
        world.request_redraw_annotations(name, core_options.map(|o| o.visible) == Some(true));
        let data_mut = get_mut(&mut world, name, "could not access data");
        let core_options_mut = get_specific_mut(f_tool_access, data_mut);
        if let Some(core_options_mut) = core_options_mut {
            core_options_mut.is_redraw_annos_triggered = false;
        }
    }
    world
}

macro_rules! released_key {
    ($($key:ident),*) => {
        #[derive(Debug, Clone, Copy)]
        pub(super) enum ReleasedKey {
            None,
            $($key,)*
        }
        pub(super) fn map_released_key(event: &Events) -> ReleasedKey {
            if false {
                ReleasedKey::None
            } $(else if event.released($crate::KeyCode::$key) {
                ReleasedKey::$key
            })*
            else {
                ReleasedKey::None
            }
        }
    };
}
macro_rules! set_cat_current {
    ($num:expr, $label_info:expr) => {
        if $num < $label_info.cat_ids().len() + 1 {
            $label_info.cat_idx_current = $num - 1;
        }
    };
}

released_key!(
    A, D, E, H, C, V, L, Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9, Delete, Back,
    Left, Right, Up, Down
);

pub fn check_recolorboxes(
    mut world: World,
    actor: &'static str,
    mut get_core_options_mut: impl FnMut(&mut World) -> Option<&mut CoreOptions>,
    mut get_label_info_mut: impl FnMut(&mut World) -> Option<&mut LabelInfo>,
) -> World {
    let is_colorchange_triggered =
        get_core_options_mut(&mut world).map(|o| o.is_colorchange_triggered);
    if is_colorchange_triggered == Some(true) {
        let core_options = get_core_options_mut(&mut world);
        if let Some(core_options) = core_options {
            core_options.is_colorchange_triggered = false;
            core_options.visible = true;
        }
        if let Some(label_info) = get_label_info_mut(&mut world) {
            label_info.new_random_colors();
        }
    }
    let are_boxes_visible = true;
    world.request_redraw_annotations(actor, are_boxes_visible);
    world
}
pub(super) fn label_change_key(key: ReleasedKey, mut label_info: LabelInfo) -> LabelInfo {
    match key {
        ReleasedKey::Key1 => {
            set_cat_current!(1, label_info);
        }
        ReleasedKey::Key2 => {
            set_cat_current!(2, label_info);
        }
        ReleasedKey::Key3 => {
            set_cat_current!(3, label_info);
        }
        ReleasedKey::Key4 => {
            set_cat_current!(4, label_info);
        }
        ReleasedKey::Key5 => {
            set_cat_current!(5, label_info);
        }
        ReleasedKey::Key6 => {
            set_cat_current!(6, label_info);
        }
        ReleasedKey::Key7 => {
            set_cat_current!(7, label_info);
        }
        ReleasedKey::Key8 => {
            set_cat_current!(8, label_info);
        }
        ReleasedKey::Key9 => {
            set_cat_current!(9, label_info);
        }
        _ => (),
    }
    label_info
}
pub(super) fn paste<T>(
    mut world: World,
    mut history: History,
    actor: &'static str,
    mut get_annos_mut: impl FnMut(&mut World) -> Option<&mut InstanceAnnotations<T>>,
    clipboard: Option<ClipboardData<T>>,
) -> (World, History)
where
    T: Annotate + Default + PartialEq + Clone,
{
    if let Some(clipboard) = &clipboard {
        let cb_bbs = clipboard.elts();
        if !cb_bbs.is_empty() {
            let shape_orig = ShapeI::from_im(world.data.im_background());
            if let Some(a) = get_annos_mut(&mut world) {
                a.extend(
                    cb_bbs.iter().cloned(),
                    clipboard.cat_idxs().iter().copied(),
                    shape_orig,
                )
            }
        }
    }
    let are_boxes_visible = true;
    world.request_redraw_annotations(actor, are_boxes_visible);
    history.push(Record::new(world.data.clone(), actor));

    (world, history)
}

pub(super) fn on_selection_keys<T>(
    mut world: World,
    mut history: History,
    key: ReleasedKey,
    is_ctrl_held: bool,
    actor: &'static str,
    mut get_annos_mut: impl FnMut(&mut World) -> Option<&mut InstanceAnnotations<T>>,
    mut get_clipboard_mut: impl FnMut(&mut World) -> Option<&mut Option<ClipboardData<T>>>,
) -> (World, History)
where
    T: Annotate + PartialEq + Default + Clone,
{
    let visible = true;
    match key {
        ReleasedKey::A if is_ctrl_held => {
            // Select all
            if let Some(a) = get_annos_mut(&mut world) {
                a.select_all()
            };
            world.request_redraw_annotations(actor, visible);
        }
        ReleasedKey::D if is_ctrl_held => {
            // Deselect all
            if let Some(a) = get_annos_mut(&mut world) {
                a.deselect_all()
            };
            world.request_redraw_annotations(actor, visible);
        }
        ReleasedKey::C if is_ctrl_held => {
            // Copy to clipboard
            let clipboard_data =
                get_annos_mut(&mut world).map(|d| ClipboardData::from_annotations(d));
            if let (Some(clipboard_data), Some(clipboard_mut)) =
                (clipboard_data, get_clipboard_mut(&mut world))
            {
                *clipboard_mut = Some(clipboard_data);
            }

            world.request_redraw_annotations(actor, visible);
        }
        ReleasedKey::V if is_ctrl_held => {
            let clipboard_data = get_clipboard_mut(&mut world).cloned().flatten();
            (world, history) = paste(world, history, actor, get_annos_mut, clipboard_data);
        }
        ReleasedKey::Delete | ReleasedKey::Back => {
            // Remove selected
            let annos = get_annos_mut(&mut world);
            if let Some(annos) = annos {
                if !annos.selected_mask().is_empty() {
                    annos.remove_selected();
                    world.request_redraw_annotations(actor, visible);
                    history.push(Record::new(world.data.clone(), actor));
                }
            }
        }
        _ => (),
    }
    (world, history)
}

pub trait Manipulate {
    fn new() -> Self
    where
        Self: Sized;

    fn on_activate(&mut self, world: World, history: History) -> (World, History) {
        (world, history)
    }
    fn on_deactivate(&mut self, world: World, history: History) -> (World, History) {
        (world, history)
    }
    fn on_filechange(&mut self, world: World, history: History) -> (World, History) {
        (world, history)
    }
    /// All events that are used by a tool are implemented in here. Use the macro [`make_tool_transform`](make_tool_transform). See, e.g.,
    /// [`Zoom::events_tf`](crate::tools::Zoom::events_tf).
    fn events_tf(&mut self, world: World, history: History, events: &Events) -> (World, History);
}

const N_HIST_ELTS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct Mover {
    mouse_pos_start: Option<PtF>,
    mouse_pos_history: [Option<PtF>; N_HIST_ELTS],
    idx_next_history_update: usize,
}
impl Mover {
    pub fn new() -> Self {
        Self {
            mouse_pos_start: None,
            mouse_pos_history: [None; N_HIST_ELTS],
            idx_next_history_update: 0,
        }
    }
    pub fn move_mouse_held<T, F: FnOnce(PtF, PtF) -> T>(
        &mut self,
        f_move: F,
        mouse_pos: Option<PtF>,
    ) -> Option<T> {
        let res = if let (Some(mp_start), Some(mp)) = (self.mouse_pos_start, mouse_pos) {
            if !self.mouse_pos_history.contains(&mouse_pos) {
                let mpo_from = Some(mp_start);
                let mpo_to = Some(mp);
                match (mpo_from, mpo_to) {
                    (Some(mp_from), Some(mp_to)) => Some(f_move(mp_from, mp_to)),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        self.mouse_pos_history[self.idx_next_history_update % N_HIST_ELTS] = self.mouse_pos_start;
        self.mouse_pos_start = mouse_pos;
        self.idx_next_history_update = self.idx_next_history_update.wrapping_add(1);
        res
    }
    pub fn move_mouse_pressed(&mut self, mouse_pos: Option<PtF>) {
        if mouse_pos.is_some() {
            self.mouse_pos_start = mouse_pos;
        }
    }
}

// applies the tool transformation to the world
#[macro_export]
macro_rules! make_tool_transform {
    (
        $self:expr,
        $world:expr,
        $history:expr,
        $event:expr,
        [$(($key_event:ident, $key_btn:expr, $method_name:ident)),*]
    ) => {
        if false {
            ($world, $history)
        }
        $(else if $event.$key_event($key_btn) {
            $self.$method_name($event, $world, $history)
        })*
        else {
            ($world, $history)
        }
    };
}
