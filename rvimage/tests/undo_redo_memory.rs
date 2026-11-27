use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};

use image::DynamicImage;
use rvlib::{
    history::{History, Record},
    world::World,
};

/// Counts the bytes currently allocated on the heap and the peak since the last reset.
struct CountingAlloc;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let current = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(current, Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

fn reset_peak() -> usize {
    let current = CURRENT.load(Ordering::Relaxed);
    PEAK.store(current, Ordering::Relaxed);
    current
}

#[test]
fn test_undo_redo_does_not_copy_image() {
    const W: u32 = 4000;
    const H: u32 = 3000;
    const IM_BYTES: usize = (W * H * 3) as usize;
    const N_EDITS: usize = 15;

    let mut world = World::empty();
    world.data.apply(|_| DynamicImage::new_rgb8(W, H));
    // the main loop takes the pending view update in each frame before tools record history
    world.update_view = Default::default();
    let mut history = History::default();

    let before_edits = reset_peak();
    for _ in 0..N_EDITS {
        history.push(Record::new(world.clone(), "test"));
    }
    let after_edits = CURRENT.load(Ordering::Relaxed);
    let peak_edits = PEAK.load(Ordering::Relaxed);

    let before_undo_redo = reset_peak();
    for _ in 0..N_EDITS - 1 {
        let (w, _) = history.prev_world(&None).expect("undo should be possible");
        world = w;
    }
    for _ in 0..N_EDITS - 1 {
        let (w, _) = history.next_world(&None).expect("redo should be possible");
        world = w;
    }
    let after_undo_redo = CURRENT.load(Ordering::Relaxed);
    let peak_undo_redo = PEAK.load(Ordering::Relaxed);
    drop(world);

    let mb = |b: usize| b as f64 / 1e6;
    println!(
        "image {:.1} MB | {N_EDITS} edits: +{:.1} MB (peak +{:.1} MB) | undo/redo: +{:.1} MB (peak +{:.1} MB)",
        mb(IM_BYTES),
        mb(after_edits.saturating_sub(before_edits)),
        mb(peak_edits - before_edits),
        mb(after_undo_redo.saturating_sub(before_undo_redo)),
        mb(peak_undo_redo - before_undo_redo),
    );
    // Without sharing the image, each history record and each undo/redo step would copy it.
    assert!(peak_edits - before_edits < IM_BYTES / 2);
    assert!(peak_undo_redo - before_undo_redo < IM_BYTES / 2);
    assert!(after_undo_redo.saturating_sub(before_undo_redo) < IM_BYTES / 2);
}
