// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Leszek Wilde

#![feature(abi_x86_interrupt)]
#![no_std]
#![no_main]

extern crate alloc;

mod allocator;
mod interrupts;
mod memory;
mod writer;

use spin::Mutex;

/// Global physical memory manager instance.
/// Encapsulated in a spinlock mutex to ensure safe access across CPU cores.
pub static PMM: Mutex<memory::BitmapAllocator> = Mutex::new(memory::BitmapAllocator::new());

/// Prints to the framebuffer via the global writer interface.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::writer::_print(format_args!($($arg)*)));
}

/// Prints to the framebuffer with an automatically appended trailing newline.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

use limine::BaseRevision;
use limine::request::{FramebufferRequest, HhdmRequest, MemmapRequest};

// Limine bootloader protocol identification structures.
// These variables must be located inside the `.requests` section and marked
// with #[used] to prevent compiler optimization discarding them.
#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemmapRequest = MemmapRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

/// Kernel execution entry point.
/// Invoked by the Limine bootloader following basic system initialization.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Assert compliance with our specified version of the Limine protocol
    assert!(BASE_REVISION.is_supported());

    // Initialize display layout and graphics pipelines
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().first() {
            let writer = writer::Writer::new(
                framebuffer.address() as *mut u8,
                framebuffer.pitch as usize,
                framebuffer.bpp as usize,
                framebuffer.width as usize,
                framebuffer.height as usize,
            );

            // Activate screen printing infrastructure
            *writer::WRITER.lock() = Some(writer);

            println!("VGOS kernel successfully booted.");
            println!(
                "Framebuffer initialized: {}x{} ({} bpp)",
                framebuffer.width, framebuffer.height, framebuffer.bpp
            );
            println!("The println! macro is fully operational.");

            // Parse physical architecture layout
            if let Some(memmap_response) = MEMORY_MAP_REQUEST.response() {
                let entries = memmap_response.entries();

                // Retrieve the Higher-Half Direct Map (HHDM) translation offset
                let hhdm_offset = if let Some(hhdm_response) = HHDM_REQUEST.response() {
                    hhdm_response.offset
                } else {
                    panic!("Fatal: Bootloader did not provide HHDM offset.");
                };

                // Initialize physical allocations based on usable RAM regions
                PMM.lock().init(entries, hhdm_offset);

                println!("\n--- Bootstrapping Global Heap ---");

                // Request a 1 Megabyte Heap allocation (256 frames of 4KB each)
                let heap_size = 1024 * 1024;
                let heap_frames = heap_size / memory::PAGE_SIZE;

                // 1. Query the PMM for a contiguous sequence of free physical frames
                if let Some(heap_phys_addr) = PMM.lock().allocate_contiguous(heap_frames) {
                    // 2. Map physical space to higher-half virtual space to avoid CPU Page Faults
                    let heap_virt_addr = heap_phys_addr + hhdm_offset as usize;

                    // 3. Bind the translated memory region to the Linked List Heap management layer
                    allocator::init_heap(heap_virt_addr, heap_size);
                    println!(
                        "Heap initialized at Virtual Address: {:#018X} ({} KB)",
                        heap_virt_addr,
                        heap_size / 1024
                    );
                } else {
                    panic!("Fatal: PMM could not find enough contiguous memory for the Heap!");
                }

                println!("\n--- Testing Rust Dynamic Types ---");

                // Verify global heap assignment using native alloc dynamic collection types
                let mut my_vec = alloc::vec::Vec::new();
                for i in 1..=5 {
                    my_vec.push(i * 10);
                }
                println!("Dynamically allocated Vec: {:?}", my_vec);

                let my_string =
                    alloc::string::String::from("Rust string living on the bare-metal heap!");
                println!("Dynamically allocated String: '{}'", my_string);

                println!("\n--- Initializing CPU Exceptions ---");
                interrupts::init_idt();
                println!("IDT Loaded.");

                println!("Triggering a fatal Page Fault...");
                unsafe {
                    // Dereferencing an arbitrary hardware address bypassing Rust's safety guarantees.
                    // Converting to a raw *mut u8 byte pointer circumvents potential alignment-induced
                    // undefined behavior checks, directly forcing the MMU to trigger an architectural
                    // Page Fault (Vector 14) upon volatile evaluation.
                    let invalid_ptr = 0xDEADBEEF as *mut u8;
                    let _value = invalid_ptr.read_volatile();
                }

                // Code execution path unreachable due to entering the unrecoverable Page Fault state.
                println!("You will never see this message!");
            } else {
                println!("Fatal: Bootloader did not provide a memory map.");
            }
        }
    }

    // Enter endless low-power wait state once structural operations finish
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}

/// System-wide unrecoverable crash hook.
/// Suspends physical processor instructions instantly.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    crate::println!("\nKERNEL PANIC!");
    crate::println!("{}", info);
    loop {
        // Safely halt the CPU to save power instead of spinning at 100% CPU utilization
        x86_64::instructions::hlt();
    }
}
