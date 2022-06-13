use crate::{util::Shape, world::World};
use image::DynamicImage;
use std::fmt::Debug;
#[derive(Clone, Default)]
pub struct History {
    ims_orig: Vec<DynamicImage>,
    current_idx: Option<usize>,
}

impl History {
    pub fn new() -> Self {
        Self {
            ims_orig: vec![],
            current_idx: None,
        }
    }
    pub fn current_world(&self) -> Option<World> {
        self.current_idx
            .map(|idx| World::new(self.ims_orig[idx].clone()))
    }
    pub fn push(&mut self, im_orig: DynamicImage) {
        match self.current_idx {
            None => {
                self.current_idx = Some(0);
                if !self.ims_orig.is_empty() {
                    self.ims_orig.clear();
                }
                self.ims_orig.push(im_orig);
            }
            Some(idx) => {
                if idx < self.ims_orig.len() - 1 {
                    self.ims_orig.truncate(idx + 1);
                }
                self.current_idx = Some(idx + 1);
                self.ims_orig.push(im_orig);
            }
        }
    }
    pub fn prev_world(&mut self, mut curr_world: DynamicImage) -> DynamicImage {
        if let Some(idx) = self.current_idx {
            if idx > 0 {
                self.current_idx = Some(idx - 1);
            } else {
                self.current_idx = None
            }
            std::mem::swap(&mut self.ims_orig[idx], &mut curr_world);
        }
        curr_world
    }
    pub fn next_world(&mut self, mut curr_world: DynamicImage) -> DynamicImage {
        match self.current_idx {
            Some(idx) if idx < self.ims_orig.len() - 1 => {
                self.current_idx = Some(idx + 1);
                std::mem::swap(&mut self.ims_orig[idx + 1], &mut curr_world);
                curr_world
            }
            None if !self.ims_orig.is_empty() => {
                self.current_idx = Some(0);
                std::mem::swap(&mut self.ims_orig[0], &mut curr_world);
                curr_world
            }
            _ => curr_world,
        }
    }
}
impl Debug for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({:?}, {:?})",
            self.current_idx,
            self.ims_orig
                .iter()
                .map(Shape::from_im)
                .collect::<Vec<_>>()
        )
    }
}
#[cfg(test)]
use crate::{result::RvResult, types::ViewImage};
#[test]
fn test_history() -> RvResult<()> {
    let im = ViewImage::new(64, 64);
    let world = World::new(DynamicImage::ImageRgb8(im));
    let mut hist = History::new();
    hist.push(world.im_orig().clone());
    let mut world = World::new(DynamicImage::ImageRgb8(ViewImage::new(32, 32)));
    hist.push(world.im_orig().clone());
    assert_eq!(hist.ims_orig.len(), 2);
    assert_eq!(hist.ims_orig[0].width(), 64);
    assert_eq!(hist.ims_orig[1].width(), 32);
    hist.prev_world(std::mem::take(world.im_orig_mut()));
    let world = World::new(DynamicImage::ImageRgb8(ViewImage::new(16, 16)));
    hist.push(world.im_orig().clone());
    assert_eq!(hist.ims_orig.len(), 2);
    assert_eq!(hist.ims_orig[0].width(), 64);
    assert_eq!(hist.ims_orig[1].width(), 16);

    Ok(())
}
