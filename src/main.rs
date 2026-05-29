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

use core::panic::PanicInfo;
use font8x8::UnicodeFonts;
use limine::BaseRevision;
use limine::request::FramebufferRequest;

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    assert!(BASE_REVISION.is_supported());

    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().first() {
            let pitch = framebuffer.pitch as usize;
            let bpp = framebuffer.bpp as usize;
            let bytes_per_pixel = bpp / 8;

            let pixel_buffer = framebuffer.address() as *mut u8;

            let text = "This is a test!";
            let mut cursor_x = 10;
            let cursor_y = 10;

            for c in text.chars() {
                if let Some(bitmap) = font8x8::BASIC_FONTS.get(c) {
                    for (row_idx, row) in bitmap.iter().enumerate() {
                        for col_idx in 0..8 {
                            if (row & (1 << col_idx)) != 0 {
                                let x = cursor_x + col_idx;
                                let y = cursor_y + row_idx;

                                let offset = (y * pitch) + (x * bytes_per_pixel);

                                unsafe {
                                    core::ptr::write_volatile(pixel_buffer.add(offset), 0xFF);
                                    core::ptr::write_volatile(pixel_buffer.add(offset + 1), 0xFF);
                                    core::ptr::write_volatile(pixel_buffer.add(offset + 2), 0xFF);
                                }
                            }
                        }
                    }
                }
                cursor_x += 8;
            }
        }
    }

    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}
