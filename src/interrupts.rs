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

use crate::println;
use spin::LazyLock;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// We use LazyLock so the IDT is only created once, safely.
static IDT: LazyLock<InterruptDescriptorTable> = LazyLock::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    // Register our handler functions
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.double_fault.set_handler_fn(double_fault_handler);

    idt
});

/// Loads our custom IDT into the CPU.
pub fn init_idt() {
    IDT.load();
}

/// Handler for the Breakpoint exception.
/// The `extern "x86-interrupt"` ABI tells the Rust compiler to generate the
/// necessary assembly to save the CPU state before running our code, and
/// restore it before returning.
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    // We can use our global macro to print the exact CPU state!
    crate::framebuffer::WRITER.lock().set_color(0x00_FF_AA_00); // Orange
    println!("EXCEPTION: BREAKPOINT");
    println!("{:#?}", stack_frame);
    crate::framebuffer::WRITER.lock().set_color(0x00_FF_FF_FF); // Back to White
}

/// Handler for a Double Fault.
/// This must return `!` (never return) because a double fault is unrecoverable.
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    // Force a kernel panic when encountering unrecoverable double faults.
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}
