//! Independent allocator evidence: track requested allocations only on this thread.
use rewind::numeric::{Array, DType, Error, GraphBfsWork, GRAPH_CHUNK, MAX_GRAPH_ITEMS};
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
fn initialization_and_endpoint_validation_do_not_materialize_million_cell_outputs() {
    let endpoints = Array::zeros(DType::Int64, vec![MAX_GRAPH_ITEMS]).unwrap();
    let (work, stats) =
        capture(|| GraphBfsWork::new(MAX_GRAPH_ITEMS, &endpoints, &endpoints, 0).unwrap());
    assert!(stats.peak < 32768, "virtual init peak {}", stats.peak);
    assert!(stats.largest < 8192);
    let (next, stats) = capture(|| work.step().unwrap());
    assert_eq!(next.phase, 0);
    assert_eq!(next.cursor, GRAPH_CHUNK);
    assert!(
        stats.peak < work.scratch_estimate(),
        "validation peak {}",
        stats.peak
    );
    assert!(stats.largest < 8192);
}
#[test]
fn scattered_adjacency_write_peak_fits_admission_and_partial_failure_is_released() {
    let n = 65536;
    let froms: Vec<i64> = (0..8193).map(|i| ((i * 257) % n) as i64).collect();
    let endpoints = Array::integers(vec![froms.len()], &froms).unwrap();
    let mut work = GraphBfsWork::new(n, &endpoints, &endpoints, 0).unwrap();
    while work.phase == 0 {
        work = work.step().unwrap();
    }
    assert_eq!(work.phase, 1);
    let admission = work.scratch_estimate();
    let (next, stats) = capture(|| work.step().unwrap());
    assert_eq!(next.phase, 1);
    assert!(
        stats.peak <= admission,
        "scatter peak {} > {}",
        stats.peak,
        admission
    );
    assert!(stats.largest < 8192);
    let mut bad_values = vec![0; GRAPH_CHUNK];
    bad_values[GRAPH_CHUNK - 1] = -1;
    let bad = Array::integers(vec![bad_values.len()], &bad_values).unwrap();
    let work = GraphBfsWork::new(n, &bad, &bad, 0).unwrap();
    let (_, stats) = capture(|| assert!(matches!(work.step(), Err(Error::Index))));
    assert_eq!(
        stats.balance, 0,
        "typed error retained temporary allocations"
    );
}

#[test]
fn huge_edgeless_first_step_reserves_only_bounded_conversion_pages() {
    let empty = Array::zeros(DType::Int64, vec![0]).unwrap();
    let work = GraphBfsWork::new(MAX_GRAPH_ITEMS, &empty, &empty, MAX_GRAPH_ITEMS - 1).unwrap();
    let admission = work.scratch_estimate();
    assert!(admission < 256 * 1024);
    let (next, stats) = capture(|| work.step().unwrap());
    assert_eq!(next.phase, 4);
    assert!(next.cursor < GRAPH_CHUNK);
    assert!(
        stats.peak <= admission,
        "edgeless peak {} > {}",
        stats.peak,
        admission
    );
    assert_eq!(work.distance.integer_flat(0).unwrap(), 0);
    assert_eq!(next.distance.integer_flat(0).unwrap(), -1);
}
