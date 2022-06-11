use crate::{
    format_rverr,
    result::{RvError, RvResult},
    world::World,
};

pub struct History {
    history: Vec<World>,
    current_world_idx: usize,
}

impl History {
    pub fn new(world: World) -> Self {
        Self {
            history: vec![world],
            current_world_idx: 0,
        }
    }
    pub fn push(&mut self, world: World) {
        if self.current_world_idx < self.history.len() - 1 {
            self.history.truncate(self.current_world_idx + 1);
        }
        self.current_world_idx += 1;
        self.history.push(world);
    }
    pub fn set_current_world_idx(&mut self, idx: usize) -> RvResult<()> {
        if idx >= self.history.len() {
            Err(format_rverr!(
                "idx {} is too hight for {} elts in history",
                idx,
                self.history.len()
            ))
        } else {
            self.current_world_idx = idx;
            Ok(())
        }
    }
    pub fn current_world_idx(&self) -> usize {
        self.current_world_idx
    }
}
#[cfg(test)]
use {crate::types::ViewImage, image::DynamicImage};
#[test]
fn test_world() -> RvResult<()> {
    let im = ViewImage::new(64, 64);
    let world = World::new(DynamicImage::ImageRgb8(im))?;
    let mut hist = History::new(world);
    let world = World::new(DynamicImage::ImageRgb8(ViewImage::new(32, 32)))?;
    hist.push(world);
    assert_eq!(hist.history.len(), 2);
    assert_eq!(hist.history[0].im_view().width(), 64);
    assert_eq!(hist.history[1].im_view().width(), 32);
    assert_eq!(hist.current_world_idx(), 1);
    assert!(hist.set_current_world_idx(2).is_err());
    hist.set_current_world_idx(0)?;
    let world = World::new(DynamicImage::ImageRgb8(ViewImage::new(16, 16)))?;
    hist.push(world);
    assert_eq!(hist.history.len(), 2);
    assert_eq!(hist.history[0].im_view().width(), 64);
    assert_eq!(hist.history[1].im_view().width(), 16);
    assert_eq!(hist.current_world_idx(), 1);

    Ok(())
}
