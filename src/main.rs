// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (C) 2026 Leszek Wilde
//
// This file is part of VGOS.
//
// VGOS is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.
//
// VGOS is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with VGOS. If not, see <https://www.gnu.org/licenses/>.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod framebuffer;
mod interrupts;

use core::panic::PanicInfo;
use limine::request::FramebufferRequest;
use limine::{BaseRevision, RequestsEndMarker, RequestsStartMarker};

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

// Enforce the Limine boot protocol base revision.
#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

// Request framebuffer initialization from the bootloader.
#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

/// Kernel entry point, called directly by the bootloader.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Verify bootloader protocol compatibility.
    assert!(BASE_REVISION.is_supported());

    // Safely retrieve the initialization response from Limine.
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.response() {
        // Ensure at least one hardware framebuffer was correctly configured.
        if let Some(framebuffer) = framebuffer_response.framebuffers().first() {
            // Map the video dimensions and base address pointer.
            let pitch = framebuffer.pitch as usize;
            let fb_ptr = framebuffer.address() as *mut u8;
            let width = framebuffer.width as usize;
            let height = framebuffer.height as usize;

            // Configure the global video output subsystem.
            framebuffer::init_global_writer(fb_ptr, pitch, width, height);

            // Log successful initialization in green text.
            framebuffer::WRITER.lock().set_color(0x00_00_FF_00);
            println!("[ OK ] Framebuffer initialized");

            // --- IDT Initialization ---
            framebuffer::WRITER.lock().set_color(0x00_FF_FF_FF);
            println!("Loading Interrupt Descriptor Table...");

            // Initialize the Interrupt Descriptor Table handlers.
            interrupts::init_idt();

            // Log successful descriptor layout configuration.
            framebuffer::WRITER.lock().set_color(0x00_00_FF_00);
            println!("[ OK ] IDT loaded successfully");

            // Reset text color back to default white.
            framebuffer::WRITER.lock().set_color(0x00_FF_FF_FF);
        }
    }

    // Await hardware interrupts.
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Handles kernel panics by permanently halting the CPU.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Highlight fatal kernel runtime panics using red text output.
    framebuffer::WRITER.lock().set_color(0x00_FF_00_00);
    println!("{}", info);

    // Safely trap execution to avoid undefined CPU states during a panic.
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
