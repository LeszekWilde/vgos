// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Leszek Wilde

use linked_list_allocator::LockedHeap;

// The #[global_allocator] attribute tells the Rust compiler to wire all
// built-in dynamic memory types (String, Vec, Box) into this specific static variable.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Initializes the global kernel heap allocator with a specified block of memory.
pub fn init_heap(heap_start: usize, heap_size: usize) {
    unsafe {
        // Hand over the designated block of physical/virtual memory to the backend allocator
        ALLOCATOR.lock().init(heap_start as *mut u8, heap_size);
    }
}
