// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Leszek Wilde

use core::fmt;
use font8x8::UnicodeFonts;
use spin::Mutex;

/// Global interface to access the framebuffer text renderer.
/// Wrapped in a Mutex to ensure thread-safe access across the kernel.
pub static WRITER: Mutex<Option<Writer>> = Mutex::new(None);

/// A primitive text renderer that draws characters directly onto a linear framebuffer.
pub struct Writer {
    framebuffer: *mut u8,
    pitch: usize,  // Number of bytes per scanline (including alignment padding)
    bpp: usize,    // Bits per pixel (e.g., 32-bit depth for ARGB arrays)
    width: usize,  // Total screen horizontal width in pixels
    height: usize, // Total screen vertical height in pixels
    cursor_x: usize,
    cursor_y: usize,
}

// Explicitly implementing Send and Sync is required because raw pointers (*mut u8)
// are not thread-safe by default. Thread safety is guaranteed via the static Mutex.
unsafe impl Send for Writer {}
unsafe impl Sync for Writer {}

impl Writer {
    /// Creates a new text renderer instance.
    /// Default screen margins are initialized at (10, 10).
    pub fn new(
        framebuffer: *mut u8,
        pitch: usize,
        bpp: usize,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            framebuffer,
            pitch,
            bpp,
            width,
            height,
            cursor_x: 10,
            cursor_y: 10,
        }
    }

    /// Renders a single ASCII/Unicode character to the screen using an 8x8 font bitmap matrix.
    fn write_char(&mut self, c: char) {
        if c == '\n' {
            self.newline();
            return;
        }

        // Handle automatic horizontal wrapping when approaching the right border
        if self.cursor_x >= self.width - 8 {
            self.newline();
        }

        let bytes_per_pixel = self.bpp / 8;

        // Fetch the 8-byte bitmap representation of the character
        if let Some(bitmap) = font8x8::BASIC_FONTS.get(c) {
            // Iterate through each row of the 8x8 font matrix
            for (row_idx, row) in bitmap.iter().enumerate() {
                // Iterate through each column bit in the active row byte
                for col_idx in 0..8 {
                    // Check if the current pixel bit state is active
                    if (row & (1 << col_idx)) != 0 {
                        let x = self.cursor_x + col_idx;
                        let y = self.cursor_y + row_idx;

                        // Calculate linear memory offset based on scanline stride pitch
                        let offset = (y * self.pitch) + (x * bytes_per_pixel);

                        // Safety: Assumes the bootloader provided valid framebuffer metrics
                        // and that the calculated target offset is within bounds.
                        unsafe {
                            // Paint pixel white (0xFF, 0xFF, 0xFF) across RGB channels
                            core::ptr::write_volatile(self.framebuffer.add(offset), 0xFF);
                            core::ptr::write_volatile(self.framebuffer.add(offset + 1), 0xFF);
                            core::ptr::write_volatile(self.framebuffer.add(offset + 2), 0xFF);
                        }
                    }
                }
            }
        }
        // Advance horizontal cursor coordinate to the next character block slot
        self.cursor_x += 8;
    }

    /// Moves the text cursor down to the next line and resets to the left margin.
    /// Wraps back to the top of the screen if the bottom boundary is crossed.
    fn newline(&mut self) {
        self.cursor_x = 10;
        self.cursor_y += 10; // 10-pixel height stride increment per row

        if self.cursor_y >= self.height - 10 {
            self.cursor_y = 10; // Fallback wrap-around layout until scrolling is available
        }
    }
}

/// Implements `core::fmt::Write` allowing the writer to use standard formatting
/// infrastructure like the `write!` macro and internal format utilities.
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

/// Helper function invoked by the public `print!` and `println!` macros.
/// Safely locks the global static instance to process format arguments.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    if let Some(writer) = WRITER.lock().as_mut() {
        writer.write_fmt(args).unwrap();
    }
}
