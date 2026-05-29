// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Leszek Wilde

#![no_std] // Kernel cannot depend on the Rust standard library
#![no_main] // Disable the standard Rust main entry point

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

use core::panic::PanicInfo;
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

                println!("\n--- Testing PMM Allocator ---");

                let mut pmm = PMM.lock();

                if let Some(addr1) = pmm.allocate_frame() {
                    println!("Allocated Frame 1 at Physical Address: {:#018X}", addr1);
                }
                if let Some(addr2) = pmm.allocate_frame() {
                    println!("Allocated Frame 2 at Physical Address: {:#018X}", addr2);
                }
                if let Some(addr3) = pmm.allocate_frame() {
                    println!("Allocated Frame 3 at Physical Address: {:#018X}", addr3);
                }
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
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}
