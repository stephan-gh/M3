/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use base::cell::StaticCell;
use base::cfg;
use base::cpu;
use base::libc;
use base::util;
use core::fmt;

pub const ISR_COUNT: usize = 66;

pub const PEX_ISR: usize = 63;
pub const TCU_ISR: usize = 64;
pub const TIMER_ISR: usize = 65;

int_enum! {
    pub struct DPL : u8 {
        const KERNEL = 0x0;
        const USER   = 0x3;
    }
}

int_enum! {
    pub struct Segment : u8 {
        const KCODE = 1;
        const KDATA = 2;
        const UCODE = 3;
        const UDATA = 4;
        const TSS = 5;
    }
}

#[derive(Default)]
// see comment in ARM code
#[repr(C, align(16))]
pub struct State {
    // general purpose registers
    pub r: [usize; 15],
    // interrupt-number
    pub irq: usize,
    // error-code (for exceptions); default = 0
    pub error: usize,
    // pushed by the CPU
    pub rip: usize,
    pub cs: usize,
    pub rflags: usize,
    pub rsp: usize,
    pub ss: usize,
}

impl State {
    pub fn came_from_user(&self) -> bool {
        (self.cs & DPL::USER.val as usize) == DPL::USER.val as usize
    }
}

fn vec_name(vec: usize) -> &'static str {
    match vec {
        0x00 => "Divide by zero",
        0x01 => "Single step",
        0x02 => "Non maskable",
        0x03 => "Breakpoint",
        0x04 => "Overflow",
        0x05 => "Bounds check",
        0x06 => "Invalid opcode",
        0x07 => "Co-proc. n/a",
        0x08 => "Double fault",
        0x09 => "Co-proc seg. overrun",
        0x0A => "Invalid TSS",
        0x0B => "Segment not present",
        0x0C => "Stack exception",
        0x0D => "Gen. prot. fault",
        0x0E => "Page fault",
        0x10 => "Co-processor error",
        _ => "<unknown>",
    }
}

impl fmt::Debug for State {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let cr2 = cpu::read_cr2();
        writeln!(fmt, "State @ {:#x}", self as *const State as usize)?;
        writeln!(fmt, "  vec: {:#x} ({})", { self.irq }, vec_name(self.irq))?;
        writeln!(fmt, "  cr2:    {:#x}", cr2)?;
        writeln!(fmt, "  error:  {:#x}", { self.error })?;
        writeln!(fmt, "  rip:    {:#x}", { self.rip })?;
        writeln!(fmt, "  rflags: {:#x}", { self.rflags })?;
        writeln!(fmt, "  rsp:    {:#x}", { self.rsp })?;
        writeln!(fmt, "  cs:     {:#x}", { self.cs })?;
        writeln!(fmt, "  ss:     {:#x}", { self.ss })?;
        for (idx, r) in { self.r }.iter().enumerate() {
            writeln!(fmt, "  r[{:02}]:  {:#x}", idx, r)?;
        }
        Ok(())
    }
}

#[repr(C, packed)]
struct DescTable {
    size: u16, // the size of the table -1 (size=0 is not allowed)
    offset: u64,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct Desc {
    // limit[0..15]
    limit_low: u16,
    // address[0..15]
    addr_low: u16,
    // address[16..23]
    addr_middle: u8,
    // type + DPL + present
    ty: u8,
    // address[24..31] and other fields, depending on the type of descriptor
    addr_high: u16,
}

int_enum! {
    struct DescType : u8 {
        const NULL            = 0x00;
        const SYS_TASK_GATE   = 0x05;
        const SYS_TSS         = 0x09;
        const SYS_INTR_GATE   = 0x0E;
        const DATA_RO         = 0x10;
        const DATA_RW         = 0x12;
        const CODE_X          = 0x18;
        const CODE_XR         = 0x1A;
    }
}

int_enum! {
    struct Bits : u8 {
        const B32 = 0 << 5;
        const B64 = 1 << 5;
    }
}

int_enum! {
    struct Size : u8 {
        const S16 = 0 << 6; // 16 bit protected mode
        const S32 = 1 << 6; // 32 bit protected mode
    }
}

int_enum! {
    struct Granularity : u8 {
        const BYTES = 0 << 7;
        const PAGES = 1 << 7;
    }
}

impl Desc {
    const fn default() -> Self {
        Self {
            addr_low: 0,
            addr_middle: 0,
            addr_high: 0,
            limit_low: 0,
            ty: 0,
        }
    }

    fn new_flat(granu: Granularity, ty: DescType, dpl: DPL) -> Self {
        Self::new(0, !0 >> cfg::PAGE_BITS, granu, ty, dpl)
    }

    fn new(addr: usize, limit: usize, granu: Granularity, ty: DescType, dpl: DPL) -> Self {
        let misc = (Bits::B64.val | Size::S16.val | granu.val) as u16;
        Self {
            addr_low: addr as u16,
            addr_middle: (addr >> 16) as u8,
            addr_high: ((addr & 0xFF00_0000) >> 16) as u16 | ((limit >> 16) & 0xF) as u16 | misc,
            limit_low: (limit & 0xFFFF) as u16,
            ty: (1 << 7) /* present */ | (dpl.val << 5) | ty.val,
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct Desc64 {
    low: Desc,
    addr_upper: u32,
    _reserved: u32,
}

type EntryFunc = unsafe extern "C" fn();

impl Desc64 {
    const fn default() -> Self {
        Self {
            low: Desc::default(),
            addr_upper: 0,
            _reserved: 0,
        }
    }

    fn new_tss(addr: usize, limit: usize, granu: Granularity, dpl: DPL) -> Self {
        Self {
            low: Desc::new(addr, limit, granu, DescType::SYS_TSS, dpl),
            addr_upper: (addr >> 32) as u32,
            _reserved: 0,
        }
    }

    fn new_idt(no: usize, handler: EntryFunc, dpl: DPL) -> Self {
        let func_addr = handler as usize;
        let present = (no != 2 && no != 15) as u8; // reserved by intel
        Self {
            low: Desc {
                addr_low: (Segment::KCODE.val as u16) << 3,
                addr_middle: 0,
                addr_high: (func_addr >> 16) as u16,
                limit_low: (func_addr & 0xFFFF) as u16,
                ty: (present << 7) | (dpl.val << 5) | DescType::SYS_INTR_GATE.val,
            },
            addr_upper: (func_addr >> 32) as u32,
            _reserved: 0,
        }
    }
}

// the Task State Segment
#[repr(C, packed)]
struct TSSInner {
    _reserved1: u32,
    rsp0: u64,
    _fields: [u32; 11],
    _reserved2: u16,
    io_bitmap: u16,
}

// we make TSSInner packed and TSS aligned to let the compiler do both
#[repr(C, align(4096))]
struct TSS {
    inner: TSSInner,
}

impl TSS {
    const fn new(rsp0: usize) -> Self {
        Self {
            inner: TSSInner {
                _reserved1: 0,
                rsp0: rsp0 as u64,
                _fields: [0; 11],
                _reserved2: 0,
                // an invalid offset for the io-bitmap => not loaded yet
                io_bitmap: 104 + 16,
            },
        }
    }
}

#[repr(C, packed)]
struct GDTInner {
    null: Desc,
    kcode: Desc,
    kdata: Desc,
    ucode: Desc,
    udata: Desc,
    tss: Desc64,
}

#[repr(C, align(8))]
struct GDT {
    inner: GDTInner,
}

impl GDT {
    const fn default() -> Self {
        Self {
            inner: GDTInner {
                null: Desc::default(),
                kcode: Desc::default(),
                kdata: Desc::default(),
                ucode: Desc::default(),
                udata: Desc::default(),
                tss: Desc64::default(),
            },
        }
    }
}

#[repr(C, align(8))]
struct IDT {
    entries: [Desc64; ISR_COUNT],
}

impl IDT {
    const fn default() -> Self {
        Self {
            entries: [Desc64::default(); ISR_COUNT],
        }
    }

    fn set(&mut self, idx: usize, handler: EntryFunc, dpl: DPL) {
        self.entries[idx] = Desc64::new_idt(idx, handler, dpl);
    }
}

extern "C" {
    fn isr_0();
    fn isr_1();
    fn isr_2();
    fn isr_3();
    fn isr_4();
    fn isr_5();
    fn isr_6();
    fn isr_7();
    fn isr_8();
    fn isr_9();
    fn isr_10();
    fn isr_11();
    fn isr_12();
    fn isr_13();
    fn isr_14();
    fn isr_15();
    fn isr_16();
    // for the exit "syscall"
    fn isr_63();
    // for the TCU
    fn isr_64();
    fn isr_65();
    // the handler for a other interrupts
    fn isr_null();
}

static GDT: StaticCell<GDT> = StaticCell::new(GDT::default());
static IDT: StaticCell<IDT> = StaticCell::new(IDT::default());
static TSS: StaticCell<TSS> = StaticCell::new(TSS::new(0));

#[no_mangle]
pub extern "C" fn isr_handler(state: &mut State) -> *mut libc::c_void {
    crate::ISRS[state.irq](state)
}

pub fn init(stack: usize) {
    set_entry_sp(stack);

    // initialize GDT
    let gdt = &mut GDT.get_mut().inner;
    gdt.kcode = Desc::new_flat(Granularity::PAGES, DescType::CODE_XR, DPL::KERNEL);
    gdt.kdata = Desc::new_flat(Granularity::PAGES, DescType::DATA_RW, DPL::KERNEL);
    gdt.ucode = Desc::new_flat(Granularity::PAGES, DescType::CODE_XR, DPL::USER);
    gdt.udata = Desc::new_flat(Granularity::PAGES, DescType::DATA_RW, DPL::USER);
    gdt.tss = Desc64::new_tss(
        TSS.get() as *const _ as *const u8 as usize,
        util::size_of::<TSSInner>() - 1,
        Granularity::BYTES,
        DPL::KERNEL,
    );

    // load GDT and TSS
    let gdt_tbl = DescTable {
        size: (util::size_of::<GDT>() - 1) as u16,
        offset: gdt as *const _ as *const u8 as u64,
    };
    let tss_off = Segment::TSS.val as usize * util::size_of::<Desc>();
    unsafe {
        llvm_asm!("lgdt ($0)" : : "r"(&gdt_tbl) : : "volatile");
        llvm_asm!("ltr $0" : : "m"(tss_off) : : "volatile");
    }

    // setup the idt
    let idt = IDT.get_mut();
    idt.set(0, isr_0, DPL::KERNEL);
    idt.set(1, isr_1, DPL::KERNEL);
    idt.set(2, isr_2, DPL::KERNEL);
    idt.set(3, isr_3, DPL::KERNEL);
    idt.set(4, isr_4, DPL::KERNEL);
    idt.set(5, isr_5, DPL::KERNEL);
    idt.set(6, isr_6, DPL::KERNEL);
    idt.set(7, isr_7, DPL::KERNEL);
    idt.set(8, isr_8, DPL::KERNEL);
    idt.set(9, isr_9, DPL::KERNEL);
    idt.set(10, isr_10, DPL::KERNEL);
    idt.set(11, isr_11, DPL::KERNEL);
    idt.set(12, isr_12, DPL::KERNEL);
    idt.set(13, isr_13, DPL::KERNEL);
    idt.set(14, isr_14, DPL::KERNEL);
    idt.set(15, isr_15, DPL::KERNEL);
    idt.set(16, isr_16, DPL::KERNEL);

    // all other interrupts
    for i in 17..=62 {
        idt.set(i, isr_null, DPL::KERNEL);
    }

    // PEMux calls
    idt.set(PEX_ISR, isr_63, DPL::USER);
    // TCU interrupts
    idt.set(TCU_ISR, isr_64, DPL::KERNEL);
    // Timer interrupts
    idt.set(TIMER_ISR, isr_65, DPL::KERNEL);

    // now we can use our idt
    let idt_tbl = DescTable {
        size: (ISR_COUNT * util::size_of::<Desc64>() - 1) as u16,
        offset: IDT.entries.as_ptr() as *const _ as *const u8 as u64,
    };
    unsafe {
        llvm_asm!("lidt ($0)" : : "r"(&idt_tbl) : : "volatile");
    }
}

pub fn set_entry_sp(sp: usize) {
    TSS.get_mut().inner.rsp0 = sp as u64;
}

pub fn enable_irqs() {
    unsafe { llvm_asm!("sti") };
}
