// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Leszek Wilde

use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
use pic8259::ChainedPics;
use spin::LazyLock;
use spin::Mutex;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// Architectural exception and hardware interrupt vector offset for the primary 8259 PIC.
/// The Intel x86_64 CPU reserves vectors 0-31 for architectural exceptions.
pub const PIC_1_OFFSET: u8 = 32;

/// Architectural exception and hardware interrupt vector offset for the secondary 8259 PIC.
/// Cascaded from the primary PIC, using the subsequent 8 vectors.
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

/// Global abstraction for the dual cascaded Intel 8259 Programmable Interrupt Controllers (PICs).
/// Encapsulated in a spinlock Mutex to ensure mutually exclusive access during runtime mask modifications.
pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

/// Enumeration of hardware interrupt vector offsets relative to the primary PIC remapping baseline.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    /// Programmable Interval Timer (IRQ 0).
    Timer = PIC_1_OFFSET,
    /// PS/2 Keyboard Controller (IRQ 1).
    Keyboard,
}

/// Global tracking state for the PS/2 keyboard protocol translation layers.
/// Encapsulated within a spinlock Mutex to permit state alteration inside asynchronous interrupt contexts.
/// Configured using a standard UK 105-key layout and Scancode Set 1 execution specifications.
pub static KEYBOARD: LazyLock<Mutex<Keyboard<layouts::Uk105Key, ScancodeSet1>>> =
    LazyLock::new(|| {
        Mutex::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Uk105Key,
            HandleControl::Ignore,
        ))
    });

impl InterruptIndex {
    /// Helper method to cast the interrupt enum variant directly into its raw 8-bit vector representation.
    fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Global Interrupt Descriptor Table (IDT).
/// Initialized lazily upon first access to prevent premature initialization
/// before the kernel's virtual memory environment is stable.
static IDT: LazyLock<InterruptDescriptorTable> = LazyLock::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.double_fault.set_handler_fn(double_fault_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);

    // Wire up hardware interrupt handlers using computed entry indexes
    idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
    idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);

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

// Interrupt handler for the Programmable Interval Timer (PIT - IRQ 0).
// Dispatched continuously based on PIT channel configurations.
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Acknowledge the hardware timer, but do not print anything.
    // Printing here can cause a permanent deadlock with the VGA text buffer's spinlock!
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

// Interrupt handler for the PS/2 Keyboard (IRQ 1).
// Fires whenever a key state transition changes on the physical controller.
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    // 1. Read the raw byte from the motherboard I/O Port 0x60 (PS/2 Data Port).
    // This action implicitly clear-acknowledges the data channel on the hardware device.
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    // 2. Lock the global keyboard state machine. Avoids data races across asynchronous paths.
    let mut keyboard = KEYBOARD.lock();

    // 3. Feed the raw byte into the state machine to track multi-byte escape sequences
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        // 4. If the byte completed a valid keypress/release sequence, decode it
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                // Renders standard ascii/unicode characters directly out to the system console
                DecodedKey::Unicode(character) => crate::print!("{}", character),

                // We intentionally ignore raw control keys so they don't print "LShift" strings
                DecodedKey::RawKey(_) => {}
            }
        }
    }

    // 5. Issue End of Interrupt (EOI) to command the primary 8259 PIC to lower its
    // In-Service Register (ISR) bit, re-enabling subsequent IRQ processing.
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
