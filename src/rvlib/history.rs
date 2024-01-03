use tracing::info;

use crate::world::World;
use std::fmt::Debug;

#[derive(Clone)]
pub struct Record {
    pub world: World,
    pub actor: &'static str,
    pub file_label_idx: Option<usize>,
    pub opened_folder: Option<String>,
}

impl Record {
    pub fn new(world: World, actor: &'static str) -> Self {
        let folder = world.data.meta_data.opened_folder.clone();
        let file_selected_idx = world.data.meta_data.file_selected_idx;

        Self {
            world,
            actor,
            file_label_idx: file_selected_idx,
            opened_folder: folder,
        }
    }

    fn convert_to_im_idx_pair(self) -> (World, Option<usize>) {
        (self.world, self.file_label_idx)
    }
}

#[derive(Clone, Default)]
pub struct History {
    records: Vec<Record>,
    current_idx: Option<usize>,
}

impl History {
    fn clear_on_folder_change(&mut self, current_folder: &Option<String>) {
        if let Some(cf) = current_folder {
            let folder_in_history = self
                .records
                .iter()
                .enumerate()
                .find(|(_, r)| r.opened_folder.as_ref() == Some(cf));
            if let Some((i, _)) = folder_in_history {
                self.records.drain(0..i);
            } else {
                self.current_idx = None;
                self.records.clear();
            }
        }
    }

    pub fn current_record(&self) -> Option<Record> {
        self.current_idx.map(|idx| self.records[idx].clone())
    }

    pub fn push(&mut self, record: Record) {
        info!("{} added to history", record.actor);
        self.clear_on_folder_change(&record.opened_folder);
        match self.current_idx {
            None => {
                self.current_idx = Some(0);
                if !self.records.is_empty() {
                    self.records.clear();
                }
                self.records.push(record);
            }
            Some(idx) => {
                if idx < self.records.len() - 1 {
                    self.records.truncate(idx + 1);
                }
                self.current_idx = Some(idx + 1);
                self.records.push(record);
            }
        }
    }

    fn change_world<F1, F2>(
        &mut self,
        idx_change: F1,
        pred: F2,
        opened_folder: &Option<String>,
    ) -> Option<(World, Option<usize>)>
    where
        F1: Fn(usize) -> usize,
        F2: FnOnce(usize) -> bool,
    {
        self.clear_on_folder_change(opened_folder);
        match self.current_idx {
            Some(idx) if pred(idx) => {
                self.current_idx = Some(idx_change(idx));
                Some(
                    self.records[idx_change(idx)]
                        .clone()
                        .convert_to_im_idx_pair(),
                )
            }
            _ => None,
        }
    }

    pub fn prev_world(&mut self, opened_folder: &Option<String>) -> Option<(World, Option<usize>)> {
        self.change_world(|idx| idx - 1, |idx| idx > 0, opened_folder)
    }

    pub fn next_world(&mut self, opened_folder: &Option<String>) -> Option<(World, Option<usize>)> {
        let n_recs = self.records.len();
        self.change_world(|idx| idx + 1, |idx| idx < n_recs - 1, opened_folder)
    }
}

impl Debug for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(record idx {:?}, {:#?})",
            self.current_idx,
            self.records
                .iter()
                .map(|r| format!(
                    "actor {}, file label idx {:?}, {:?}, folder label {:?}",
                    r.actor,
                    r.file_label_idx,
                    &r.world.data.shape(),
                    r.opened_folder
                ))
                .collect::<Vec<_>>()
        )
    }
}

#[cfg(test)]
use {
    crate::{result::RvResult, types::ViewImage},
    image::DynamicImage,
    std::collections::HashMap,
};
#[test]
fn test_history() -> RvResult<()> {
    let im = ViewImage::new(64, 64);
    let world = World::from_real_im(DynamicImage::ImageRgb8(im), HashMap::new(), None, None);
    let mut hist = History::default();

    hist.push(Record {
        world: world.clone(),
        actor: "",
        file_label_idx: None,
        opened_folder: None,
    });
    let world = World::from_real_im(
        DynamicImage::ImageRgb8(ViewImage::new(32, 32)),
        HashMap::new(),
        None,
        None,
    );
    hist.push(Record {
        world: world.clone(),
        actor: "",
        file_label_idx: None,
        opened_folder: None,
    });
    assert_eq!(hist.records.len(), 2);
    assert_eq!(hist.records[0].world.data.shape().w, 64);
    assert_eq!(hist.records[1].world.data.shape().w, 32);
    hist.prev_world(&None);
    let world = World::from_real_im(
        DynamicImage::ImageRgb8(ViewImage::new(16, 16)),
        HashMap::new(),
        None,
        None,
    );
    hist.push(Record {
        world: world.clone(),
        actor: "",
        file_label_idx: None,
        opened_folder: None,
    });
    assert_eq!(hist.records.len(), 2);
    assert_eq!(hist.records[0].world.data.shape().w, 64);
    assert_eq!(hist.records[1].world.data.shape().w, 16);

    hist.push(Record {
        world: world.clone(),
        actor: "",
        file_label_idx: None,
        opened_folder: Some("folder1".to_string()),
    });
    assert_eq!(hist.records.len(), 1);

    hist.push(Record {
        world: world.clone(),
        actor: "",
        file_label_idx: None,
        opened_folder: Some("folder2".to_string()),
    });
    hist.push(Record {
        world: world.clone(),
        actor: "",
        file_label_idx: None,
        opened_folder: None,
    });
    hist.push(Record {
        world: world.clone(),
        actor: "",
        file_label_idx: None,
        opened_folder: Some("folder2".to_string()),
    });

    assert_eq!(hist.records.len(), 3);
    assert_eq!(hist.records[0].opened_folder, Some("folder2".to_string()));
    assert_eq!(hist.records[1].opened_folder, None);
    assert_eq!(hist.records[2].opened_folder, Some("folder2".to_string()));

    Ok(())
}
