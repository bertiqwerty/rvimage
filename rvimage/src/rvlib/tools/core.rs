use tracing::{info, warn};

use super::attributes;
use crate::history::Record;
use crate::result::trace_ok_err;
use crate::tools_data::annotations::{ClipboardData, InstanceAnnotations};
use crate::tools_data::attributes_data::{self, AttrVal};
use crate::tools_data::{
    self, get_mut, get_specific_mut, vis_from_lfoption, CoreOptions, InstanceAnnotate, LabelInfo,
    ToolSpecifics,
};
use crate::ShapeI;
use crate::{events::Events, history::History, world::World};
use rvimage_domain::{PtF, RvResult};
use std::mem;
pub(super) fn make_track_changes_str(actor: &'static str) -> String {
    format!("{actor}_TRACK_CHANGE")
}

pub(super) fn change_annos<T>(
    world: &mut World,
    track_change_str: &str,
    track_changes: bool,
    f_change: impl FnOnce(&mut InstanceAnnotations<T>),
    get_annos_mut: impl Fn(&mut World) -> Option<&mut InstanceAnnotations<T>>,
) {
    if let Some(annos) = get_annos_mut(world) {
        f_change(annos);
    }
    if track_changes {
        let mut old_attr_name = String::new();
        let mut old_attr_type = AttrVal::Bool(false);
        let track_change_str = track_change_str.to_string();
        if let Ok(attr_data) =
            tools_data::get_mut(world, attributes::ACTOR_NAME, "Attr data missing")
        {
            let populate_new_attr = attr_data
                .specifics
                .attributes()
                .map(|a| a.attr_names().contains(&track_change_str))
                != Ok(true);

            trace_ok_err(attr_data.specifics.attributes_mut().map(|d| {
                let attr_options = attributes_data::Options {
                    is_export_triggered: false,
                    is_addition_triggered: populate_new_attr,
                    is_update_triggered: false,
                    removal_idx: None,
                };
                old_attr_name = d.new_attr.clone();
                old_attr_type = d.new_attr_type.clone();
                d.new_attr = track_change_str.to_string();
                d.new_attr_type = AttrVal::Bool(false);
                d.options = attr_options;
            }));
        }
        (*world, _) = attributes::Attributes {}.events_tf(
            mem::take(world),
            History::default(),
            &Events::default(),
        );
        if let Ok(attr_data) =
            tools_data::get_mut(world, attributes::ACTOR_NAME, "Attr data missing")
        {
            let attr_options = attributes_data::Options {
                is_export_triggered: false,
                is_addition_triggered: false,
                is_update_triggered: true,
                removal_idx: None,
            };
            trace_ok_err(attr_data.specifics.attributes_mut().map(|d| {
                d.options = attr_options;
                if let Some(attr_map) = &mut d.current_attr_map {
                    attr_map.insert(track_change_str, AttrVal::Bool(true));
                } else {
                    warn!("no attrmap found");
                }
            }));
        }
        (*world, _) = attributes::Attributes {}.events_tf(
            mem::take(world),
            History::default(),
            &Events::default(),
        );
        if let Ok(attr_data) =
            tools_data::get_mut(world, attributes::ACTOR_NAME, "Attr data missing")
        {
            trace_ok_err(attr_data.specifics.attributes_mut().map(|d| {
                d.new_attr = old_attr_name;
                d.new_attr_type = old_attr_type;
            }));
        }
    }
}
pub(super) fn check_trigger_redraw(
    mut world: World,
    name: &'static str,
    get_label_info: impl Fn(&World) -> Option<&LabelInfo>,
    f_tool_access: impl FnMut(&mut ToolSpecifics) -> RvResult<&mut CoreOptions> + Clone,
) -> World {
    let data_mut = get_mut(&mut world, name, "could not access data");
    let core_options = get_specific_mut(f_tool_access.clone(), data_mut).cloned();
    let is_redraw_triggered = core_options.map(|o| o.is_redraw_annos_triggered);
    if is_redraw_triggered == Some(true) {
        let visibility = vis_from_lfoption(
            get_label_info(&world),
            core_options.map(|o| o.visible) == Some(true),
        );
        world.request_redraw_annotations(name, visibility);
        let data_mut = get_mut(&mut world, name, "could not access data");
        let core_options_mut = get_specific_mut(f_tool_access, data_mut);
        if let Some(core_options_mut) = core_options_mut {
            core_options_mut.is_redraw_annos_triggered = false;
        }
    }
    world
}

pub(super) fn check_trigger_history_update(
    mut world: World,
    mut history: History,
    name: &'static str,
    f_tool_access: impl FnMut(&mut ToolSpecifics) -> RvResult<&mut CoreOptions> + Clone,
) -> (World, History) {
    let data_mut = get_mut(&mut world, name, "could not access data");
    let core_options = get_specific_mut(f_tool_access.clone(), data_mut).cloned();
    let is_history_update_triggered = core_options.map(|o| o.is_history_update_triggered);
    if is_history_update_triggered == Some(true) {
        let data_mut = get_mut(&mut world, name, "could not access data");
        let core_options_mut = get_specific_mut(f_tool_access, data_mut);
        if let Some(core_options_mut) = core_options_mut {
            core_options_mut.is_history_update_triggered = false;
        }
        history.push(Record::new(world.clone(), name));
    }
    (world, history)
}

macro_rules! event_2_actionenum {
    ($name:ident, $func:ident, $map_func:ident, $($key:ident),*) => {
        #[derive(Debug, Clone, Copy)]
        pub(super) enum $name {
            None,
            $($key,)*
        }
        pub(super) fn $map_func(event: &Events) -> $name {
            if false {
                $name::None
            } $(else if event.$func($crate::KeyCode::$key) {
                $name::$key
            })*
            else {
                $name::None
            }
        }
    };
}

event_2_actionenum!(
    ReleasedKey,
    released,
    map_released_key,
    A,
    D,
    E,
    H,
    C,
    I,
    T,
    V,
    L,
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Delete,
    Back,
    Left,
    Right,
    Up,
    Down
);
event_2_actionenum!(HeldKey, held, map_held_key, I, T);
macro_rules! set_cat_current {
    ($num:expr, $label_info:expr) => {
        if $num < $label_info.cat_ids().len() + 1 {
            if $label_info.cat_idx_current == $num - 1 {
                $label_info.show_only_current = !$label_info.show_only_current;
            } else {
                $label_info.cat_idx_current = $num - 1;
                $label_info.show_only_current = false;
            }
            true
        } else {
            false
        }
    };
}

pub(super) fn check_autopaste<T>(
    mut world: World,
    mut history: History,
    actor: &'static str,
    mut get_options_mut: impl FnMut(&mut World) -> Option<&mut CoreOptions>,
    get_annos_mut: impl Fn(&mut World) -> Option<&mut InstanceAnnotations<T>>,
    get_label_info: impl Fn(&World) -> Option<&LabelInfo>,
    get_clipboard: impl Fn(&World) -> Option<ClipboardData<T>>,
) -> (World, History)
where
    T: InstanceAnnotate,
{
    let auto_paste = get_options_mut(&mut world)
        .map(|o| o.auto_paste)
        .unwrap_or(false);
    if world.data.meta_data.is_loading_screen_active == Some(false) && auto_paste {
        let annos = get_annos_mut(&mut world);
        if let Some(annos) = annos {
            let all = (0..annos.elts().len()).collect::<Vec<_>>();
            annos.remove_multiple(&all);
        }
        let clipboard = get_clipboard(&world);
        (world, history) = paste(
            world,
            history,
            actor,
            clipboard,
            get_annos_mut,
            get_options_mut,
            get_label_info,
        );
    }
    (world, history)
}
pub fn check_erase_mode(
    released_key: ReleasedKey,
    get_options_mut: impl Fn(&mut World) -> Option<&mut CoreOptions>,
    set_visible: impl Fn(&mut World),
    mut world: World,
) -> World {
    if let (ReleasedKey::E, Some(core_options)) = (released_key, get_options_mut(&mut world)) {
        if core_options.erase {
            info!("stop erase via shortcut");
        } else {
            info!("start erase via shortcut");
        }
        core_options.visible = true;
        core_options.erase = !core_options.erase;
        set_visible(&mut world);
    }
    world
}

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
        let visibility = vis_from_lfoption(get_label_info_mut(&mut world).map(|x| &*x), true);
        world.request_redraw_annotations(actor, visibility);
    }
    world
}
pub(super) fn label_change_key(key: ReleasedKey, mut label_info: LabelInfo) -> (LabelInfo, bool) {
    let label_change = match key {
        ReleasedKey::Key1 => {
            set_cat_current!(1, label_info)
        }
        ReleasedKey::Key2 => {
            set_cat_current!(2, label_info)
        }
        ReleasedKey::Key3 => {
            set_cat_current!(3, label_info)
        }
        ReleasedKey::Key4 => {
            set_cat_current!(4, label_info)
        }
        ReleasedKey::Key5 => {
            set_cat_current!(5, label_info)
        }
        ReleasedKey::Key6 => {
            set_cat_current!(6, label_info)
        }
        ReleasedKey::Key7 => {
            set_cat_current!(7, label_info)
        }
        ReleasedKey::Key8 => {
            set_cat_current!(8, label_info)
        }
        ReleasedKey::Key9 => {
            set_cat_current!(9, label_info)
        }
        _ => false,
    };
    (label_info, label_change)
}
pub(super) fn paste<T>(
    mut world: World,
    mut history: History,
    actor: &'static str,
    clipboard: Option<ClipboardData<T>>,
    get_annos_mut: impl Fn(&mut World) -> Option<&mut InstanceAnnotations<T>>,
    mut get_options_mut: impl FnMut(&mut World) -> Option<&mut CoreOptions>,
    get_label_info: impl Fn(&World) -> Option<&LabelInfo>,
) -> (World, History)
where
    T: InstanceAnnotate,
{
    if let Some(clipboard) = &clipboard {
        let cb_bbs = clipboard.elts();
        if !cb_bbs.is_empty() {
            let shape_orig = ShapeI::from_im(world.data.im_background());
            let paste_annos = |a: &mut InstanceAnnotations<T>| {
                a.extend(
                    cb_bbs.iter().cloned(),
                    clipboard.cat_idxs().iter().copied(),
                    shape_orig,
                )
            };
            let track_changes = get_options_mut(&mut world).map(|o| o.track_changes) == Some(true);
            change_annos(
                &mut world,
                &make_track_changes_str(actor),
                track_changes,
                paste_annos,
                get_annos_mut,
            );
        }
        let options_mut = get_options_mut(&mut world);
        if let Some(options_mut) = options_mut {
            options_mut.visible = true;
        }
        let visible = get_options_mut(&mut world).map(|o| o.visible) == Some(true);
        let vis = vis_from_lfoption(get_label_info(&world), visible);
        world.request_redraw_annotations(actor, vis);
        history.push(Record::new(world.clone(), actor));
    }

    (world, history)
}
pub fn deselect_all<T>(
    mut world: World,
    actor: &'static str,
    get_annos_mut: impl Fn(&mut World) -> Option<&mut InstanceAnnotations<T>>,
    get_label_info: impl Fn(&World) -> Option<&LabelInfo>,
) -> World
where
    T: InstanceAnnotate,
{
    // Deselect all
    if let Some(a) = get_annos_mut(&mut world) {
        a.deselect_all()
    };
    let vis = vis_from_lfoption(get_label_info(&world), true);
    world.request_redraw_annotations(actor, vis);
    world
}

#[allow(clippy::too_many_arguments)]
pub(super) fn on_selection_keys<T>(
    mut world: World,
    mut history: History,
    key: ReleasedKey,
    is_ctrl_held: bool,
    actor: &'static str,
    get_annos_mut: impl Fn(&mut World) -> Option<&mut InstanceAnnotations<T>>,
    get_clipboard_mut: impl Fn(&mut World) -> Option<&mut Option<ClipboardData<T>>>,
    mut get_options_mut: impl FnMut(&mut World) -> Option<&mut CoreOptions>,
    get_label_info: impl Fn(&World) -> Option<&LabelInfo>,
) -> (World, History)
where
    T: InstanceAnnotate,
{
    match key {
        ReleasedKey::A if is_ctrl_held => {
            // Select all visible
            let current_active_idx = get_label_info(&world).and_then(|li| {
                if li.show_only_current {
                    Some(li.cat_idx_current)
                } else {
                    None
                }
            });
            let options = get_options_mut(&mut world);
            if options.map(|o| o.visible) == Some(true) {
                if let (Some(current_active), Some(a)) =
                    (current_active_idx, get_annos_mut(&mut world))
                {
                    let relevant_indices = a
                        .cat_idxs()
                        .iter()
                        .enumerate()
                        .filter(|(_, cat_idx)| **cat_idx == current_active)
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>();
                    a.select_multi(relevant_indices.into_iter());
                } else if let Some(a) = get_annos_mut(&mut world) {
                    a.select_all()
                };
                let vis = vis_from_lfoption(get_label_info(&world), true);
                world.request_redraw_annotations(actor, vis);
            }
        }
        ReleasedKey::D if is_ctrl_held => {
            world = deselect_all(world, actor, get_annos_mut, get_label_info);
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
            let vis = vis_from_lfoption(get_label_info(&world), true);
            world.request_redraw_annotations(actor, vis);
        }
        ReleasedKey::V if is_ctrl_held => {
            let clipboard_data = get_clipboard_mut(&mut world).cloned().flatten();
            (world, history) = paste(
                world,
                history,
                actor,
                clipboard_data,
                get_annos_mut,
                get_options_mut,
                get_label_info,
            );
        }
        ReleasedKey::V if !is_ctrl_held => {
            if let Some(options_mut) = get_options_mut(&mut world) {
                options_mut.auto_paste = !options_mut.auto_paste;
            }
        }
        ReleasedKey::Delete | ReleasedKey::Back => {
            // Remove selected

            let track_changes = get_options_mut(&mut world).map(|o| o.track_changes) == Some(true);
            let del_annos = |annos: &mut InstanceAnnotations<T>| {
                if !annos.selected_mask().is_empty() {
                    annos.remove_selected();
                }
            };
            let track_change_str = make_track_changes_str(actor);
            change_annos(
                &mut world,
                track_change_str.as_str(),
                track_changes,
                del_annos,
                get_annos_mut,
            );
            let vis = vis_from_lfoption(get_label_info(&world), true);
            world.request_redraw_annotations(actor, vis);
            history.push(Record::new(world.clone(), actor));
        }
        _ => (),
    }
    (world, history)
}

pub trait Manipulate {
    fn new() -> Self
    where
        Self: Sized;

    fn on_activate(&mut self, world: World) -> World {
        world
    }
    fn on_deactivate(&mut self, world: World) -> World {
        world
    }
    fn on_filechange(&mut self, world: World, history: History) -> (World, History) {
        (world, history)
    }
    fn on_always_active_zoom(&mut self, world: World, history: History) -> (World, History) {
        (world, history)
    }
    /// None -> the tool does not tell you if it has been used
    /// Some(true) -> the tool has been used
    /// Some(false) -> the tool has not been used
    fn has_been_used(&self, _: &Events) -> Option<bool> {
        None
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
