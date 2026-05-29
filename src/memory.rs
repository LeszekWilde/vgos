// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Leszek Wilde

use limine::memmap::Entry;

/// The fixed size of a single physical memory page frame (4KB).
pub const PAGE_SIZE: usize = 4096;

/// A physical memory manager that tracks page frame availability using a bitmap tracking array.
pub struct BitmapAllocator {
    bitmap_ptr: *mut u8,
    bitmap_length: usize,
    total_frames: usize,
    free_frames: usize,
}

// Ensure our static instance is thread-safe across execution contexts
unsafe impl Send for BitmapAllocator {}
unsafe impl Sync for BitmapAllocator {}

impl BitmapAllocator {
    /// Creates an uninitialized instance of the bitmap allocator.
    pub const fn new() -> Self {
        Self {
            bitmap_ptr: core::ptr::null_mut(),
            bitmap_length: 0,
            total_frames: 0,
            free_frames: 0,
        }
    }

    /// Initializes the bitmap tracking array using the hardware memory map and virtual offset.
    pub fn init(&mut self, memmap: &[&Entry], hhdm_offset: u64) {
        let mut top_address: usize = 0;

        // Scan the entire memory map to find the highest physical address boundary
        for entry in memmap {
            let end = (entry.base + entry.length) as usize;
            if end > top_address {
                top_address = end;
            }
        }

        self.total_frames = top_address / PAGE_SIZE;
        self.bitmap_length = align_up(self.total_frames / 8, PAGE_SIZE);

        crate::println!("Total System Frames: {}", self.total_frames);
        crate::println!("Required Bitmap Size: {} bytes", self.bitmap_length);

        // Find a usable region of memory large enough to store our tracking bitmap
        for entry in memmap {
            if entry.type_ == 0 && entry.length as usize >= self.bitmap_length {
                // Apply the higher-half offset to calculate the virtual memory pointer
                let physical_addr = entry.base;
                let virtual_addr = physical_addr + hhdm_offset;

                self.bitmap_ptr = virtual_addr as *mut u8;
                break;
            }
        }

        if self.bitmap_ptr.is_null() {
            panic!("Fatal: Not enough contiguous memory to store the PMM bitmap!");
        }

        crate::println!(
            "Bitmap allocated at Virtual Address: {:#018X}",
            self.bitmap_ptr as usize
        );

        // Initially mark all frames as allocated/locked (0xFF) to ensure safety
        unsafe {
            core::ptr::write_bytes(self.bitmap_ptr, 0xFF, self.bitmap_length);
        }

        self.free_frames = 0;

        // Liberate pages that map directly to usable RAM chunks (type 0)
        for entry in memmap {
            if entry.type_ == 0 {
                self.free_region(entry.base as usize, entry.length as usize);
            }
        }

        // Re-lock the physical memory region that holds our newly written allocation bitmap
        let physical_bitmap_addr = (self.bitmap_ptr as u64 - hhdm_offset) as usize;
        self.lock_region(physical_bitmap_addr, self.bitmap_length);

        crate::println!("PMM Bootstrapped. Total Free Frames: {}", self.free_frames);
        crate::println!(
            "Available RAM: {} MB",
            (self.free_frames * PAGE_SIZE) / 1024 / 1024
        );
    }

    /// Finds the first available 4KB frame, locks it, and returns its physical address.
    pub fn allocate_frame(&mut self) -> Option<usize> {
        if self.free_frames == 0 {
            return None; // Out of physical memory
        }

        // Scan bitmap tracking segments to find a byte containing unallocated bits (not 0xFF)
        for byte_idx in 0..self.bitmap_length {
            unsafe {
                let byte_ptr = self.bitmap_ptr.add(byte_idx);
                let byte = byte_ptr.read();

                if byte != 0xFF {
                    // Identify the exact free bit position within the target byte
                    for bit_idx in 0..8 {
                        if (byte & (1 << bit_idx)) == 0 {
                            let frame = (byte_idx * 8) + bit_idx;

                            // Mark the frame as locked/used
                            self.lock_frame(frame);

                            // Compute and return the corresponding absolute physical address
                            return Some(frame * PAGE_SIZE);
                        }
                    }
                }
            }
        }

        None
    }

    /// Finds a contiguous block of N free frames and returns the physical base address.
    pub fn allocate_contiguous(&mut self, num_frames: usize) -> Option<usize> {
        if self.free_frames < num_frames {
            return None;
        }

        let mut current_consecutive = 0;
        let mut start_frame = 0;

        // Scan the entire bitmap bit-by-bit to find a contiguous free block
        for frame in 0..self.total_frames {
            let byte_idx = frame / 8;
            let bit_idx = frame % 8;

            let is_used = unsafe {
                let byte = self.bitmap_ptr.add(byte_idx).read();
                (byte & (1 << bit_idx)) != 0
            };

            if is_used {
                current_consecutive = 0;
                start_frame = frame + 1; // Reset our search to the next available frame index
            } else {
                current_consecutive += 1;

                // Match identified for requested allocation span layout
                if current_consecutive == num_frames {
                    // Lock all tracking bits encompassing this structural slice
                    for i in 0..num_frames {
                        self.lock_frame(start_frame + i);
                    }
                    return Some(start_frame * PAGE_SIZE);
                }
            }
        }
        None // Target alignment block is omitted or memory is too fragmented
    }

    /// Internal helper to set a specific frame's tracking bit to 0 (Free).
    fn free_frame(&mut self, frame: usize) {
        let byte_idx = frame / 8;
        let bit_idx = frame % 8;

        if byte_idx < self.bitmap_length {
            unsafe {
                let byte_ptr = self.bitmap_ptr.add(byte_idx);
                let current_byte = byte_ptr.read();
                // Clear target bit using bitwise AND against an inverted mask
                byte_ptr.write(current_byte & !(1 << bit_idx));
            }
            self.free_frames += 1;
        }
    }

    /// Internal helper to set a specific frame's tracking bit to 1 (Used/Locked).
    fn lock_frame(&mut self, frame: usize) {
        let byte_idx = frame / 8;
        let bit_idx = frame % 8;

        if byte_idx < self.bitmap_length {
            unsafe {
                let byte_ptr = self.bitmap_ptr.add(byte_idx);
                let current_byte = byte_ptr.read();
                // Set target bit using bitwise OR
                byte_ptr.write(current_byte | (1 << bit_idx));
            }
            self.free_frames -= 1;
        }
    }

    /// Frees an entire contiguous region of physical memory frames.
    fn free_region(&mut self, base: usize, length: usize) {
        let start_frame = base / PAGE_SIZE;
        let frames_to_free = length / PAGE_SIZE;

        for i in 0..frames_to_free {
            self.free_frame(start_frame + i);
        }
    }

    /// Locks an entire contiguous region of physical memory frames.
    fn lock_region(&mut self, base: usize, length: usize) {
        let start_frame = base / PAGE_SIZE;
        let frames_to_lock = align_up(length, PAGE_SIZE) / PAGE_SIZE;

        for i in 0..frames_to_lock {
            self.lock_frame(start_frame + i);
        }
    }
}

/// Helper function to align an address UP to the nearest multiple of `align`.
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
