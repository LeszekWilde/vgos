// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Leszek Wilde

use spin::LazyLock;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// Global Interrupt Descriptor Table (IDT).
/// Initialized lazily upon first access to prevent premature initialization
/// before the kernel's virtual memory environment is stable.
static IDT: LazyLock<InterruptDescriptorTable> = LazyLock::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.double_fault.set_handler_fn(double_fault_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);

    idt
});

/// Loads the Interrupt Descriptor Table (IDT) into the CPU's LIDT register.
pub fn init_idt() {
    IDT.load();
}

// Exception handler for software breakpoints (INT3).
// This handler prints the hardware stack frame context and safely returns execution
// to the instruction immediately following the break instruction.
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    crate::println!("\n*** CPU EXCEPTION: BREAKPOINT ***");
    crate::println!("{:#?}", stack_frame);
    crate::println!("*********************************");
}

// Exception handler for Double Faults (CPU Vector 8).
// Double faults occur when a secondary exception triggers during the delivery
// of a primary exception. This is a non-recoverable architectural failure;
// therefore, the handler is marked as a diverging function ('!') and panics.
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

// Exception handler for Page Faults (CPU Vector 14).
// Triggered when a memory access violates page translation rules or access privileges.
// Reads the CR2 register to capture the precise faulting virtual address.
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    crate::println!("\n*** CPU EXCEPTION: PAGE FAULT ***");

    // CR2 register contains the architectural virtual address that caused the fault
    crate::println!("Accessed Address: {:?}", Cr2::read());
    crate::println!("Error Code: {:?}", error_code);
    crate::println!("{:#?}", stack_frame);
    crate::println!("*********************************");

    // Execution cannot safely resume without resolving the mapping page tables.
    // In this basic bootstrap phase, we place the core into an infinite low-power halt state.
    loop {
        x86_64::instructions::hlt();
    }
}
