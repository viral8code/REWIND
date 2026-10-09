//! Independent allocator evidence: track requested allocations only on this thread.
use rewind::numeric::{Array, DType, Error, SumShapeProgress, SHAPE_CHUNK};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

#[derive(Clone, Copy)]
struct Stats {
    active: bool,
    largest: usize,
    balance: isize,
    peak: usize,
}
thread_local! { static STATS: Cell<Stats> = const { Cell::new(Stats { active: false, largest: 0, balance: 0, peak: 0 }) }; }
struct Allocator;
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn update(allocated: usize, released: usize) {
    let _ = STATS.try_with(|cell| {
        let mut value = cell.get();
        if value.active {
            value.largest = value.largest.max(allocated);
            value.balance += allocated as isize - released as isize;
            value.peak = value.peak.max(value.balance.max(0) as usize);
            cell.set(value);
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            update(layout.size(), 0);
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            update(layout.size(), 0);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        update(0, layout.size());
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            update(size, layout.size());
        }
        next
    }
}
fn capture<T>(run: impl FnOnce() -> T) -> (T, Stats) {
    STATS.with(|cell| {
        cell.set(Stats {
            active: true,
            largest: 0,
            balance: 0,
            peak: 0,
        })
    });
    let result = run();
    let stats = STATS.with(|cell| {
        let value = cell.get();
        cell.set(Stats {
            active: false,
            ..value
        });
        value
    });
    (result, stats)
}

#[test]
fn bounded_copy_and_contraction_footprints_fit_admission_without_full_temporaries() {
    let n = 1_048_576;
    let input = Array::zeros(DType::Float64, vec![n])
        .unwrap()
        .slice(0, n - 1, n, -1)
        .unwrap();
    let output = input.reshape_logical_init(vec![1024, 1024]).unwrap();
    let (step, stats) = capture(|| input.reshape_logical_step(&output, 0).unwrap());
    assert_eq!(step.1, SHAPE_CHUNK);
    assert!(!step.2);
    assert!(stats.peak < 256 * 1024);
    assert!(stats.largest < 8192);
    let p = SumShapeProgress::new(&input, vec![n]).unwrap();
    let reserve = p.scratch_estimate(&input).unwrap();
    assert!(reserve < 512 * 1024);
    let (next, stats) = capture(|| p.step(&input).unwrap());
    assert!(stats.peak <= reserve);
    assert!(stats.largest < 8192);
    assert_eq!(next.cursor, SHAPE_CHUNK);
    assert_eq!(next.sums.float_flat(0).unwrap(), 0.0);
}
#[test]
fn wrapped_contraction_page_footprint_and_error_cleanup_are_accounted() {
    let n = 8193;
    let values = vec![1.0; n * 3];
    let input = Array::floats(vec![3, n], &values).unwrap();
    let mut p = SumShapeProgress::new(&input, vec![n]).unwrap();
    p = p.step(&input).unwrap();
    p = p.step(&input).unwrap();
    assert_eq!(p.cursor, SHAPE_CHUNK * 2);
    let reserve = p.scratch_estimate(&input).unwrap();
    let (_, stats) = capture(|| p.step(&input).unwrap());
    assert!(
        stats.peak <= reserve,
        "wrapped peak {} > {}",
        stats.peak,
        reserve
    );
    let mut values = vec![1.0; SHAPE_CHUNK];
    values[SHAPE_CHUNK - 1] = f64::INFINITY;
    let input = Array::floats(vec![values.len()], &values).unwrap();
    let p = SumShapeProgress::new(&input, vec![values.len()]).unwrap();
    let (_, stats) = capture(|| assert!(matches!(p.step(&input), Err(Error::NonFinite))));
    assert_eq!(stats.balance, 0);
}
