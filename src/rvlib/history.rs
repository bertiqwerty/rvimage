use crate::world::ImsRaw;
use std::fmt::Debug;

#[derive(Clone)]
pub struct Record {
    pub ims_raw: ImsRaw,
    pub file_label_idx: Option<usize>,
    pub folder_label: Option<String>,
}
impl Record {
    pub fn new(ims_raw: ImsRaw) -> Self {
        Self {
            ims_raw,
            file_label_idx: None,
            folder_label: None,
        }
    }
    fn convert_to_im_idx_pair(self) -> (ImsRaw, Option<usize>) {
        (self.ims_raw, self.file_label_idx)
    }
}
#[derive(Clone, Default)]
pub struct History {
    records: Vec<Record>,
    current_idx: Option<usize>,
}

impl History {
    pub fn new() -> Self {
        Self {
            records: vec![],
            current_idx: None,
        }
    }
    fn clear_on_folder_change(&mut self, current_folder_label: &Option<String>) {
        if let Some(cfl) = current_folder_label {
            let folder_in_history = self
                .records
                .iter()
                .enumerate()
                .find(|(_, r)| r.folder_label.as_ref() == Some(cfl));
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
        self.clear_on_folder_change(&record.folder_label);
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

    pub fn prev_world(&mut self, mut curr_world: Record) -> (ImsRaw, Option<usize>) {
        self.clear_on_folder_change(&curr_world.folder_label);
        if let Some(idx) = self.current_idx {
            if idx > 0 {
                self.current_idx = Some(idx - 1);
            } else {
                self.current_idx = None
            }
            std::mem::swap(&mut self.records[idx], &mut curr_world);
        }
        curr_world.convert_to_im_idx_pair()
    }

    pub fn next_world(&mut self, mut curr_world: Record) -> (ImsRaw, Option<usize>) {
        self.clear_on_folder_change(&curr_world.folder_label);
        match self.current_idx {
            Some(idx) if idx < self.records.len() - 1 => {
                self.current_idx = Some(idx + 1);
                std::mem::swap(&mut self.records[idx + 1], &mut curr_world);
                curr_world.convert_to_im_idx_pair()
            }
            None if !self.records.is_empty() => {
                self.current_idx = Some(0);
                std::mem::swap(&mut self.records[0], &mut curr_world);
                curr_world.convert_to_im_idx_pair()
            }
            _ => curr_world.convert_to_im_idx_pair(),
        }
    }
}
impl Debug for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(record idx {:?}, {:?})",
            self.current_idx,
            self.records
                .iter()
                .map(|r| format!(
                    "file label idx {:?} - {:?}",
                    r.file_label_idx,
                    &r.ims_raw.shape()
                ))
                .collect::<Vec<_>>()
        )
    }
}
#[cfg(test)]
use {
    crate::{util::Shape, result::RvResult, types::ViewImage, world::World},
    image::DynamicImage,
};
#[test]
fn test_history() -> RvResult<()> {
    let dummy_shape_win = Shape::new(128, 128);
    let im = ViewImage::new(64, 64);
    let world = World::from_im(DynamicImage::ImageRgb8(im), dummy_shape_win);
    let mut hist = History::new();
    
    hist.push(Record {
        ims_raw: world.ims_raw.clone(),
        file_label_idx: None,
        folder_label: None,
    });
    let mut world = World::from_im(DynamicImage::ImageRgb8(ViewImage::new(32, 32)), dummy_shape_win);
    hist.push(Record {
        ims_raw: world.ims_raw.clone(),
        file_label_idx: None,
        folder_label: None,
    });
    assert_eq!(hist.records.len(), 2);
    assert_eq!(hist.records[0].ims_raw.shape().w, 64);
    assert_eq!(hist.records[1].ims_raw.shape().w, 32);
    hist.prev_world(Record {
        ims_raw: std::mem::take(&mut world.ims_raw),
        file_label_idx: None,
        folder_label: None,
    });
    let world = World::from_im(DynamicImage::ImageRgb8(ViewImage::new(16, 16)), dummy_shape_win);
    hist.push(Record {
        ims_raw: world.ims_raw.clone(),
        file_label_idx: None,
        folder_label: None,
    });
    assert_eq!(hist.records.len(), 2);
    assert_eq!(hist.records[0].ims_raw.shape().w, 64);
    assert_eq!(hist.records[1].ims_raw.shape().w, 16);

    hist.push(Record {
        ims_raw: world.ims_raw.clone(),
        file_label_idx: None,
        folder_label: Some("folder1".to_string()),
    });
    assert_eq!(hist.records.len(), 1);

    hist.push(Record {
        ims_raw: world.ims_raw.clone(),
        file_label_idx: None,
        folder_label: Some("folder2".to_string()),
    });
    hist.push(Record {
        ims_raw: world.ims_raw.clone(),
        file_label_idx: None,
        folder_label: None,
    });
    hist.push(Record {
        ims_raw: world.ims_raw.clone(),
        file_label_idx: None,
        folder_label: Some("folder2".to_string()),
    });

    assert_eq!(hist.records.len(), 3);
    assert_eq!(hist.records[0].folder_label, Some("folder2".to_string()));
    assert_eq!(hist.records[1].folder_label, None);
    assert_eq!(hist.records[2].folder_label, Some("folder2".to_string()));

    Ok(())
}
