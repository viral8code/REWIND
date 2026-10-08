//! Independent allocator evidence: track requested allocations only on this thread.
use rewind::numeric::{Array, DType, Error};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

#[derive(Clone, Copy)]
struct Stats {
    active: bool,
    largest: usize,
    balance: isize,
}
thread_local! { static STATS: Cell<Stats> = const { Cell::new(Stats { active: false, largest: 0, balance: 0 }) }; }
struct Allocator;
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn update(allocated: usize, released: usize) {
    let _ = STATS.try_with(|cell| {
        let mut value = cell.get();
        if value.active {
            value.largest = value.largest.max(allocated);
            value.balance += allocated as isize - released as isize;
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
fn map_and_zip_outputs_avoid_a_full_size_temporary_allocation() {
    let length = 1_048_576;
    let float = Array::zeros(DType::Float64, vec![length]).unwrap();
    let (mapped, stats) = capture(|| float.map_float(|x| Ok(x + 1.0)));
    assert!(
        stats.largest < length,
        "largest requested allocation {} bytes",
        stats.largest
    );
    assert_eq!(mapped.unwrap().float_flat(length - 1).unwrap(), 1.0);
    let (zipped, stats) = capture(|| float.zip_float(&float, |a, b| Ok(a + b)));
    assert!(
        stats.largest < length,
        "largest requested allocation {} bytes",
        stats.largest
    );
    assert_eq!(zipped.unwrap().float_flat(length - 1).unwrap(), 0.0);
    let integer = Array::zeros(DType::Int64, vec![length]).unwrap();
    let (zipped, stats) = capture(|| integer.zip_integer(&integer, i64::checked_add));
    assert!(
        stats.largest < length,
        "largest requested allocation {} bytes",
        stats.largest
    );
    assert_eq!(zipped.unwrap().integer_flat(length - 1).unwrap(), 0);
}

#[test]
fn late_error_releases_all_allocations_made_for_partial_output() {
    let input = Array::zeros(DType::Float64, vec![16384]).unwrap();
    let mut calls = 0;
    let (result, stats) = capture(|| {
        input.map_float(|x| {
            calls += 1;
            if calls == 8194 {
                Err(Error::Domain)
            } else {
                Ok(x + 1.0)
            }
        })
    });
    assert!(matches!(result, Err(Error::Domain)));
    assert_eq!(calls, 8194);
    assert_eq!(
        stats.balance, 0,
        "partial output retained allocator-requested bytes"
    );
    assert_eq!(input.float_flat(8193).unwrap(), 0.0);
}
