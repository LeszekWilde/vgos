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

use core::fmt;
use font8x8::UnicodeFonts;
use spin::{LazyLock, Mutex};

/// A global, thread-safe instance of the framebuffer Writer.
/// It is initially uninitialized and must be configured via `init_global_writer`
/// before calling any print macros.
pub static WRITER: LazyLock<Mutex<Writer>> = LazyLock::new(|| Mutex::new(Writer::uninitialized()));

/// Initializes the global WRITER with the bootloader's framebuffer data.
/// This MUST be called inside `_start` before any printing happens.
pub fn init_global_writer(fb_ptr: *mut u8, pitch: usize, width: usize, height: usize) {
    let mut writer = WRITER.lock();
    *writer = Writer::new(fb_ptr, pitch, width, height);
}

/// A text renderer that manages writing characters directly to a raw pixel framebuffer.
pub struct Writer {
    fb_ptr: *mut u8,
    pitch: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    color: u32,
    scale: usize,
    initialized: bool,
}

// Ensure the `Writer` state can be safely transferred and accessed across threads.
unsafe impl Send for Writer {}
unsafe impl Sync for Writer {}

impl Writer {
    /// Creates an empty placeholder Writer. Used by the Lazy static initialization.
    const fn uninitialized() -> Self {
        Self {
            fb_ptr: core::ptr::null_mut(),
            pitch: 0,
            width: 0,
            height: 0,
            x: 0,
            y: 0,
            color: 0x00_FF_FF_FF,
            scale: 2,
            initialized: false,
        }
    }

    /// Creates a new `Writer` instance tied to a specific linear framebuffer.
    pub fn new(fb_ptr: *mut u8, pitch: usize, width: usize, height: usize) -> Self {
        Self {
            fb_ptr,
            pitch,
            width,
            height,
            x: 0,
            y: 0,
            color: 0x00_FF_FF_FF, // Default to solid white
            scale: 2,             // Default text scaling factor
            initialized: true,
        }
    }

    /// Sets the text color using an ARGB/XRGB 32-bit format.
    pub fn set_color(&mut self, color: u32) {
        self.color = color;
    }

    /// Sets the text scale factor to adjust font dimensions.
    pub fn set_scale(&mut self, scale: usize) {
        self.scale = scale;
    }

    /// Draws a single pixel directly into the memory-mapped framebuffer.
    fn draw_pixel(&mut self, x: usize, y: usize, color: u32) {
        // Prevent writing out of the physical screen bounds.
        if x >= self.width || y >= self.height {
            return;
        }

        // Calculate byte offset: (row * row size in bytes) + (column * bytes per pixel)
        let pixel_offset = (y * self.pitch) + (x * 4);

        // Perform a volatile write to bypass compiler optimizations on MMIO.
        unsafe {
            let color_ptr = self.fb_ptr.add(pixel_offset) as *mut u32;
            color_ptr.write_volatile(color);
        }
    }

    /// Renders a single character using the 8x8 bitmap font.
    pub fn write_char(&mut self, c: char) {
        // Handle explicit newline sequences.
        if c == '\n' {
            self.new_line();
            return;
        }

        // Wrap lines automatically if the next character exceeds the screen width.
        if self.x + (8 * self.scale) >= self.width {
            self.new_line();
        }

        // Fetch and parse the glyph bitmap representation.
        if let Some(bitmap) = font8x8::BASIC_FONTS.get(c) {
            for (row_idx, row_byte) in bitmap.iter().enumerate() {
                for col_idx in 0..8 {
                    // Check if the current bit in the glyph matrix is set.
                    if (*row_byte & (1 << col_idx)) != 0 {
                        // Render a scaled macro-pixel for the font.
                        for sy in 0..self.scale {
                            for sx in 0..self.scale {
                                self.draw_pixel(
                                    self.x + (col_idx * self.scale) + sx,
                                    self.y + (row_idx * self.scale) + sy,
                                    self.color,
                                );
                            }
                        }
                    }
                }
            }
        }

        // Advance the cursor position by the character width.
        self.x += 8 * self.scale;
    }

    /// Moves the cursor down to the start of the next line.
    fn new_line(&mut self) {
        self.x = 0;
        self.y += 8 * self.scale;
    }

    /// Iterates over and prints a string slice.
    pub fn write_string(&mut self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
        }
    }
}

/// Implements standard formatting traits to allow usage with macros like `write!` and `writeln!`.
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Standard macro for printing text output through the global kernel writer.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::framebuffer::_print(format_args!($($arg)*)));
}

/// Standard macro for printing text output with an appended newline character.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    // Lock the spin mutex, write the formatted arguments, and release the lock.
    WRITER.lock().write_fmt(args).unwrap();
}
