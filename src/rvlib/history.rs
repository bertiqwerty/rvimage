use crate::util::Shape;
use image::DynamicImage;
use std::fmt::Debug;

#[derive(Clone)]
pub struct Record {
    pub im_orig: DynamicImage,
    pub file_label_idx: Option<usize>,
    pub folder_label: Option<String>,
}
impl Record {
    fn move_to_im_idx_pair(self) -> (DynamicImage, Option<usize>) {
        (self.im_orig, self.file_label_idx)
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

    pub fn prev_world(&mut self, mut curr_world: Record) -> (DynamicImage, Option<usize>) {
        self.clear_on_folder_change(&curr_world.folder_label);
        if let Some(idx) = self.current_idx {
            if idx > 0 {
                self.current_idx = Some(idx - 1);
            } else {
                self.current_idx = None
            }
            std::mem::swap(&mut self.records[idx], &mut curr_world);
        }
        curr_world.move_to_im_idx_pair()
    }

    pub fn next_world(&mut self, mut curr_world: Record) -> (DynamicImage, Option<usize>) {
        self.clear_on_folder_change(&curr_world.folder_label);
        match self.current_idx {
            Some(idx) if idx < self.records.len() - 1 => {
                self.current_idx = Some(idx + 1);
                std::mem::swap(&mut self.records[idx + 1], &mut curr_world);
                curr_world.move_to_im_idx_pair()
            }
            None if !self.records.is_empty() => {
                self.current_idx = Some(0);
                std::mem::swap(&mut self.records[0], &mut curr_world);
                curr_world.move_to_im_idx_pair()
            }
            _ => curr_world.move_to_im_idx_pair(),
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
                    Shape::from_im(&r.im_orig)
                ))
                .collect::<Vec<_>>()
        )
    }
}
#[cfg(test)]
use crate::{result::RvResult, types::ViewImage, world::World};
#[test]
fn test_history() -> RvResult<()> {
    let im = ViewImage::new(64, 64);
    let world = World::new(DynamicImage::ImageRgb8(im));
    let mut hist = History::new();

    hist.push(Record {
        im_orig: world.im_orig().clone(),
        file_label_idx: None,
        folder_label: None,
    });
    let mut world = World::new(DynamicImage::ImageRgb8(ViewImage::new(32, 32)));
    hist.push(Record {
        im_orig: world.im_orig().clone(),
        file_label_idx: None,
        folder_label: None,
    });
    assert_eq!(hist.records.len(), 2);
    assert_eq!(hist.records[0].im_orig.width(), 64);
    assert_eq!(hist.records[1].im_orig.width(), 32);
    hist.prev_world(Record {
        im_orig: std::mem::take(world.im_orig_mut()),
        file_label_idx: None,
        folder_label: None,
    });
    let world = World::new(DynamicImage::ImageRgb8(ViewImage::new(16, 16)));
    hist.push(Record {
        im_orig: world.im_orig().clone(),
        file_label_idx: None,
        folder_label: None,
    });
    assert_eq!(hist.records.len(), 2);
    assert_eq!(hist.records[0].im_orig.width(), 64);
    assert_eq!(hist.records[1].im_orig.width(), 16);

    hist.push(Record {
        im_orig: world.im_orig().clone(),
        file_label_idx: None,
        folder_label: Some("folder1".to_string()),
    });
    assert_eq!(hist.records.len(), 1);

    hist.push(Record {
        im_orig: world.im_orig().clone(),
        file_label_idx: None,
        folder_label: Some("folder2".to_string()),
    });
    hist.push(Record {
        im_orig: world.im_orig().clone(),
        file_label_idx: None,
        folder_label: None,
    });
    hist.push(Record {
        im_orig: world.im_orig().clone(),
        file_label_idx: None,
        folder_label: Some("folder2".to_string()),
    });

    assert_eq!(hist.records.len(), 3);
    assert_eq!(hist.records[0].folder_label, Some("folder2".to_string()));
    assert_eq!(hist.records[1].folder_label, None);
    assert_eq!(hist.records[2].folder_label, Some("folder2".to_string()));

    Ok(())
}
