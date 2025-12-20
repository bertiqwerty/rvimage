use tracing::{info, warn};

use super::attributes;
use crate::history::Record;
use crate::result::trace_ok_err;
use crate::tools_data::annotations::{ClipboardData, InstanceAnnotations};
use crate::tools_data::attributes_data;
use crate::tools_data::parameters::ParamVal;
use crate::tools_data::{ImportExportTrigger, InstanceAnnotate, LabelInfo, vis_from_lfoption};
use crate::util::Visibility;
use crate::world::InstanceAnnoAccess;
use crate::{InstanceLabelDisplay, ShapeI};
use crate::{
    events::Events,
    history::History,
    world,
    world::{MetaDataAccess, World},
};
use rvimage_domain::PtF;
use std::mem;

pub(super) fn make_track_changes_str(actor: &'static str) -> String {
    format!("{actor}_TRACK_CHANGE")
}
pub(super) fn insert_attribute(
    mut world: World,
    attr_name: &str,
    value: ParamVal,
    default_value: ParamVal,
    filepath: Option<&str>,
) -> World {
    let mut old_attr_name = String::new();
    let mut old_attr_val = ParamVal::Bool(false);

    if let Ok(attr_data) = world::get_mut(&mut world, attributes::ACTOR_NAME, "Attr data missing") {
        // does the new attribute already exist?
        let populate_new_attr = attr_data.specifics.attributes().map(|a| {
            a.attr_names()
                .iter()
                .any(|attr_name| attr_name.as_str() == attr_name)
        }) != Ok(true);

        // set the attribute data to addtion
        trace_ok_err(attr_data.specifics.attributes_mut().map(|d| {
            let attr_options = attributes_data::Options {
                import_export_trigger: ImportExportTrigger::default(),
                is_addition_triggered: populate_new_attr,
                rename_src_idx: None,
                is_update_triggered: false,
                export_only_opened_folder: false,
                removal_idx: None,
            };
            old_attr_name.clone_from(&d.new_attr_name);
            old_attr_val.clone_from(&d.new_attr_val);
            d.new_attr_name = attr_name.to_string();
            d.new_attr_val = default_value;
            d.options = attr_options;
        }));
    }

    // actually add the attribute
    (world, _) = attributes::Attributes {}.events_tf(world, History::default(), &Events::default());

    // insert the attribute's value to the attribute map of the current file
    if let Ok(attr_data) = world::get_mut(&mut world, attributes::ACTOR_NAME, "Attr data missing") {
        let attr_options = attributes_data::Options {
            import_export_trigger: ImportExportTrigger::default(),
            is_addition_triggered: false,
            rename_src_idx: None,
            is_update_triggered: true,
            export_only_opened_folder: false,
            removal_idx: None,
        };
        trace_ok_err(attr_data.specifics.attributes_mut().map(|d| {
            d.options = attr_options;
            let attr_map = if let Some(filepath) = filepath {
                d.attr_map_mut(filepath)
            } else {
                d.current_attr_map.as_mut()
            };
            if let Some(attr_map) = attr_map {
                attr_map.insert(attr_name.to_string(), value);
            } else {
                warn!("no attrmap found");
            }
        }));
    }
    (world, _) = attributes::Attributes {}.events_tf(world, History::default(), &Events::default());

    if let Ok(attr_data) = world::get_mut(&mut world, attributes::ACTOR_NAME, "Attr data missing") {
        // reset the state of the attribute data
        trace_ok_err(attr_data.specifics.attributes_mut().map(|d| {
            d.new_attr_name = old_attr_name;
            d.new_attr_val = old_attr_val;
        }));
    }

    world
}

pub(super) fn change_annos<T, DA, IA>(
    world: &mut World,
    f_change: impl FnOnce(&mut InstanceAnnotations<T>),
) where
    T: InstanceAnnotate,
    DA: MetaDataAccess,
    IA: InstanceAnnoAccess<T>,
{
    if let Some(annos) = IA::get_annos_mut(world) {
        f_change(annos);
    }
    let track_changes_str = DA::get_track_changes_str(world);
    if let Some(track_changes_str) = track_changes_str {
        *world = insert_attribute(
            mem::take(world),
            track_changes_str,
            ParamVal::Bool(true),
            ParamVal::Bool(false),
            None,
        );
    }
}
pub(super) fn check_trigger_redraw<DC>(mut world: World, name: &'static str) -> World
where
    DC: MetaDataAccess,
{
    let core_options = DC::get_core_options(&world).copied();
    let is_redraw_triggered = core_options.map(|o| o.is_redraw_annos_triggered);
    if is_redraw_triggered == Some(true) {
        let visibility = vis_from_lfoption(
            DC::get_label_info(&world),
            core_options.map(|o| o.visible) == Some(true),
        );
        world.request_redraw_annotations(name, visibility);
        let core_options_mut = DC::get_core_options_mut(&mut world);
        if let Some(core_options_mut) = core_options_mut {
            core_options_mut.is_redraw_annos_triggered = false;
        }
    }
    world
}

pub(super) fn check_trigger_history_update<DC>(
    mut world: World,
    mut history: History,
    name: &'static str,
) -> (World, History)
where
    DC: MetaDataAccess,
{
    let core_options = DC::get_core_options_mut(&mut world).copied();
    let is_history_update_triggered = core_options.map(|o| o.is_history_update_triggered);
    if is_history_update_triggered == Some(true) {
        let core_options_mut = DC::get_core_options_mut(&mut world);
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

fn replace_annotations_with_clipboard<T, DA, IA>(
    mut world: World,
    history: History,
    actor: &'static str,
    clipboard: Option<&ClipboardData<T>>,
) -> (World, History)
where
    T: InstanceAnnotate,
    DA: MetaDataAccess,
    IA: InstanceAnnoAccess<T>,
{
    let annos = IA::get_annos_mut(&mut world);
    if let Some(annos) = annos {
        let all = (0..annos.elts().len()).collect::<Vec<_>>();
        annos.remove_multiple(&all);
    }
    paste::<T, DA, IA>(world, history, actor, clipboard)
}
pub(super) fn check_autopaste<T, DA, IA>(
    mut world: World,
    mut history: History,
    actor: &'static str,
) -> (World, History)
where
    T: InstanceAnnotate,
    DA: MetaDataAccess,
    IA: InstanceAnnoAccess<T>,
{
    let clipboard_data = IA::get_clipboard(&world).cloned();
    let auto_paste = DA::get_core_options_mut(&mut world).is_some_and(|o| o.auto_paste);
    if world.data.meta_data.flags.is_loading_screen_active == Some(false) && auto_paste {
        history.push(Record::new(world.clone(), actor));
        replace_annotations_with_clipboard::<T, DA, IA>(
            world,
            history,
            actor,
            clipboard_data.as_ref(),
        )
    } else {
        (world, history)
    }
}
pub fn check_erase_mode<AC>(
    released_key: ReleasedKey,
    set_visible: impl Fn(&mut World),
    mut world: World,
) -> World
where
    AC: MetaDataAccess,
{
    if let (ReleasedKey::E, Some(core_options)) =
        (released_key, AC::get_core_options_mut(&mut world))
    {
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

pub fn check_recolorboxes<AC>(mut world: World, actor: &'static str) -> World
where
    AC: MetaDataAccess,
{
    let is_colorchange_triggered =
        AC::get_core_options_mut(&mut world).map(|o| o.is_colorchange_triggered);
    if is_colorchange_triggered == Some(true) {
        let core_options = AC::get_core_options_mut(&mut world);
        if let Some(core_options) = core_options {
            core_options.is_colorchange_triggered = false;
            core_options.visible = true;
        }
        if let Some(label_info) = AC::get_label_info_mut(&mut world) {
            label_info.new_random_colors();
        }
        let visibility = vis_from_lfoption(AC::get_label_info_mut(&mut world).map(|x| &*x), true);
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
pub(super) fn paste<T, DA, IA>(
    mut world: World,
    mut history: History,
    actor: &'static str,
    clipboard: Option<&ClipboardData<T>>,
) -> (World, History)
where
    T: InstanceAnnotate,
    DA: MetaDataAccess,
    IA: InstanceAnnoAccess<T>,
{
    if let Some(clipboard) = clipboard {
        let cb_bbs = clipboard.elts();
        if !cb_bbs.is_empty() {
            let ild = DA::get_core_options(&world)
                .map(|o| o.instance_label_display)
                .unwrap_or_default();
            let shape_orig = ShapeI::from_im(world.data.im_background());
            let paste_annos = |a: &mut InstanceAnnotations<T>| {
                a.extend(
                    cb_bbs.iter().cloned(),
                    clipboard.cat_idxs().iter().copied(),
                    shape_orig,
                    ild,
                );
            };
            change_annos::<T, DA, IA>(&mut world, paste_annos);
        }
        let options_mut = DA::get_core_options_mut(&mut world);
        if let Some(options_mut) = options_mut {
            options_mut.visible = true;
        }
        let visible = DA::get_core_options_mut(&mut world).map(|o| o.visible) == Some(true);
        let vis = vis_from_lfoption(DA::get_label_info(&world), visible);
        world.request_redraw_annotations(actor, vis);
        history.push(Record::new(world.clone(), actor));
    }

    (world, history)
}
pub fn deselect_all<T, DA, IA>(mut world: World, actor: &'static str) -> World
where
    T: InstanceAnnotate,
    DA: MetaDataAccess,
    IA: InstanceAnnoAccess<T>,
{
    // Deselect all
    if let Some(a) = IA::get_annos_mut(&mut world) {
        a.deselect_all();
    };
    let vis = vis_from_lfoption(DA::get_label_info(&world), true);
    world.request_redraw_annotations(actor, vis);
    world
}

pub(super) fn instance_label_display_sort<T, DA, IA>(
    mut world: World,
    instance_label_display: InstanceLabelDisplay,
    actor: &'static str,
) -> World
where
    T: InstanceAnnotate,
    DA: MetaDataAccess,
    IA: InstanceAnnoAccess<T>,
{
    let annos = IA::get_annos_mut(&mut world);
    if let Some(annos) = annos {
        *annos = instance_label_display.sort(mem::take(annos));
    }

    let vis = vis_from_lfoption(DA::get_label_info(&world), true);
    world.request_redraw_annotations(actor, vis);

    world
}
pub(super) fn check_instance_label_display_change<T, DA, IA>(
    mut world: World,
    key: ReleasedKey,
    actor: &'static str,
) -> World
where
    T: InstanceAnnotate,
    DA: MetaDataAccess,
    IA: InstanceAnnoAccess<T>,
{
    if let ReleasedKey::L = key {
        // update instance label display
        let options = DA::get_core_options_mut(&mut world);
        if let Some(options) = options {
            options.instance_label_display = options.instance_label_display.next();
            tracing::info!(
                "instance label display changed to {}",
                options.instance_label_display
            );
        }

        // sort the annotations
        let ild = DA::get_core_options(&world)
            .map(|o| o.instance_label_display)
            .unwrap_or_default();
        world = instance_label_display_sort::<_, DA, IA>(world, ild, actor);
    }
    world
}

pub(super) fn on_selection_keys<T, DA, IA>(
    mut world: World,
    mut history: History,
    key: ReleasedKey,
    is_ctrl_held: bool,
    actor: &'static str,
) -> (World, History)
where
    T: InstanceAnnotate,
    DA: MetaDataAccess,
    IA: InstanceAnnoAccess<T>,
{
    match key {
        ReleasedKey::A if is_ctrl_held => {
            // Select all visible
            let current_active_idx = DA::get_label_info(&world).and_then(|li| {
                if li.show_only_current {
                    Some(li.cat_idx_current)
                } else {
                    None
                }
            });
            let options = DA::get_core_options_mut(&mut world);
            if options.map(|o| o.visible) == Some(true) {
                if let (Some(current_active), Some(a)) =
                    (current_active_idx, IA::get_annos_mut(&mut world))
                {
                    let relevant_indices = a
                        .cat_idxs()
                        .iter()
                        .enumerate()
                        .filter(|(_, cat_idx)| **cat_idx == current_active)
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>();
                    a.select_multi(relevant_indices.into_iter());
                } else if let Some(a) = IA::get_annos_mut(&mut world) {
                    a.select_all();
                };
                let vis = vis_from_lfoption(DA::get_label_info(&world), true);
                world.request_redraw_annotations(actor, vis);
            }
        }
        ReleasedKey::D if is_ctrl_held => {
            world = deselect_all::<_, DA, IA>(world, actor);
        }
        ReleasedKey::C if is_ctrl_held => {
            // Copy to clipboard
            let clipboard_data_new =
                IA::get_annos(&world).map(|d| ClipboardData::from_annotations(d));
            IA::set_clipboard(&mut world, clipboard_data_new);
            let vis = vis_from_lfoption(DA::get_label_info(&world), true);
            world.request_redraw_annotations(actor, vis);
        }
        ReleasedKey::V if is_ctrl_held => {
            let clipboard_data = IA::get_clipboard(&world).cloned();
            (world, history) = paste::<_, DA, IA>(world, history, actor, clipboard_data.as_ref());
        }
        ReleasedKey::V if !is_ctrl_held => {
            let clipboard_data = IA::get_clipboard(&world).cloned();
            if let Some(options_mut) = DA::get_core_options_mut(&mut world) {
                options_mut.auto_paste = !options_mut.auto_paste;
                if options_mut.auto_paste {
                    (world, history) = replace_annotations_with_clipboard::<T, DA, IA>(
                        world,
                        history,
                        actor,
                        clipboard_data.as_ref(),
                    );
                }
            }
        }
        ReleasedKey::Delete | ReleasedKey::Back => {
            // Remove selected
            let del_annos = |annos: &mut InstanceAnnotations<T>| {
                if !annos.selected_mask().is_empty() {
                    annos.remove_selected();
                }
            };
            change_annos::<T, DA, IA>(&mut world, del_annos);
            let vis = vis_from_lfoption(DA::get_label_info(&world), true);
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

    fn get_visibility(&self, _world: &World) -> Visibility {
        Visibility::None
    }
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
            if self.mouse_pos_history.contains(&mouse_pos) {
                None
            } else {
                let mpo_from = Some(mp_start);
                let mpo_to = Some(mp);
                match (mpo_from, mpo_to) {
                    (Some(mp_from), Some(mp_to)) => Some(f_move(mp_from, mp_to)),
                    _ => None,
                }
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
        $events:expr,
        [$(($key_event:ident, $key_btn:expr, $method_name:ident)),*]
    ) => {
        if false {
            ($world, $history)
        }
        $(else if $events.$key_event($key_btn) {
            $self.$method_name($events, $world, $history)
        })*
        else {
            ($world, $history)
        }
    };
}
