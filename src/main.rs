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

mod framebuffer;

use core::panic::PanicInfo;
use limine::request::FramebufferRequest;
use limine::{BaseRevision, RequestsEndMarker, RequestsStartMarker};

use core::fmt::Write;

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

            // Instantiate the kernel-space screen renderer.
            let mut writer = framebuffer::Writer::new(fb_ptr, pitch, width, height);

            // Print the initial operating system greeting.
            let _ = writeln!(writer, "Welcome to VGOS!");

            // Log successful initialisation in green text.
            writer.set_color(0x00_00_FF_00);
            let _ = writeln!(
                writer,
                "[ OK ] Framebuffer initialised ({}x{})",
                width, height
            );

            // Reset text color to default white for upcoming status logs.
            writer.set_color(0x00_FF_FF_FF);
            let _ = writeln!(writer, "[INFO] Ready for next subsystem...");
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
fn panic(_info: &PanicInfo) -> ! {
    // Safely trap execution to avoid undefined CPU states during a panic.
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
