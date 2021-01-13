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

#![feature(llvm_asm)]
#![no_std]

extern crate heap;

mod paging;
mod pes;

use base::boxed::Box;
use base::cell::{LazyStaticCell, StaticCell};
use base::cfg;
use base::col::Vec;
use base::io;
use base::kif::{PageFlags, Perm};
use base::libc;
use base::log;
use base::machine;
use base::math::next_log2;
use base::read_csr;
use base::tcu::{self, EpId, Message, Reg, EP_REGS, TCU};
use base::util;
use base::vec;

use pes::PE;

static LOG_DEF: bool = true;

static OWN_VPE: u16 = 0xFFFF;
static STATE: LazyStaticCell<isr::State> = LazyStaticCell::default();
static XLATES: StaticCell<u64> = StaticCell::new(0);

extern "C" {
    fn heap_init(begin: usize, end: usize);
}

#[no_mangle]
pub extern "C" fn abort() {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) {
    machine::shutdown();
}

pub extern "C" fn mmu_pf(state: &mut isr::State) -> *mut libc::c_void {
    let virt = read_csr!("stval");

    let perm = match isr::Vector::from(state.cause & 0x1F) {
        isr::Vector::INSTR_PAGEFAULT => PageFlags::R | PageFlags::X,
        isr::Vector::LOAD_PAGEFAULT => PageFlags::R,
        isr::Vector::STORE_PAGEFAULT => PageFlags::R | PageFlags::W,
        _ => unreachable!(),
    };

    panic!(
        "Pagefault for address={:#x}, perm={:?} with {:?}",
        virt, perm, state
    );
}

pub extern "C" fn tcu_irq(state: &mut isr::State) -> *mut libc::c_void {
    tcu::TCU::clear_irq(tcu::IRQ::CORE_REQ);

    // core request from TCU?
    if let Some(r) = tcu::TCU::get_core_req() {
        log!(crate::LOG_DEF, "Got {:x?}", r);
        match r {
            tcu::CoreReq::Xlate(r) => {
                XLATES.set(*XLATES + 1);

                let pte = paging::translate(r.virt, r.perm);
                // no page faults supported
                assert!(!(pte & PageFlags::RW.bits()) & r.perm.bits() == 0);
                log!(crate::LOG_DEF, "TCU can continue with PTE={:#x}", pte);
                tcu::TCU::set_xlate_resp(pte);
            },
            tcu::CoreReq::Foreign(_) => panic!("Unexpected message for foreign VPE"),
        }
    }

    state as *mut _ as *mut libc::c_void
}

fn config_local_ep<CFG>(ep: EpId, cfg: CFG)
where
    CFG: FnOnce(&mut [Reg]),
{
    let mut regs = [0 as Reg; EP_REGS];
    cfg(&mut regs);
    TCU::set_ep_regs(ep, &regs);
}

fn test_mem(size_in: usize) {
    log!(crate::LOG_DEF, "READ+WRITE with {} 8B words", size_in);

    let _dummy = vec![0u8; size_in * cfg::PAGE_SIZE];

    let mut buffer = vec![0u64; size_in];

    // prepare test data
    let mut data = Vec::with_capacity(size_in);
    for i in 0..size_in {
        data.push(i as u64);
    }

    // configure mem EP
    let data_size = data.len() * util::size_of::<u64>();
    config_local_ep(1, |regs| {
        TCU::config_mem(regs, OWN_VPE, PE::MEM.id(), 0x1000, data_size, Perm::RW);
    });

    // test write + read
    TCU::write(1, data.as_ptr() as *const u8, data_size, 0).unwrap();
    TCU::read(1, buffer.as_mut_ptr() as *mut u8, data_size, 0).unwrap();

    assert_eq!(buffer, data);
}

fn test_msgs(size_in: usize) {
    let virt_to_phys = |virt: usize| -> (usize, ::paging::Phys) {
        let rbuf_pte = paging::translate(virt, PageFlags::R);
        (
            virt,
            (rbuf_pte & !cfg::PAGE_MASK as u64) + (virt & cfg::PAGE_MASK) as u64,
        )
    };

    let fetch_msg = |ep: EpId, rbuf: usize| -> Option<&'static Message> {
        tcu::TCU::fetch_msg(ep).map(|off| tcu::TCU::offset_to_msg(rbuf, off))
    };

    log!(crate::LOG_DEF, "SEND+REPLY with {} 8B words", size_in);

    let _dummy = vec![0u8; size_in * cfg::PAGE_SIZE];

    // create receive buffers
    let rbuf1 = vec![0u8; 64];
    let (rbuf1_virt, rbuf1_phys) = virt_to_phys(rbuf1.as_ptr() as usize);
    let rbuf2 = vec![0u8; 64];
    let (rbuf2_virt, rbuf2_phys) = virt_to_phys(rbuf2.as_ptr() as usize);

    // create EPs
    config_local_ep(1, |regs| {
        TCU::config_recv(
            regs,
            OWN_VPE,
            rbuf1_phys,
            next_log2(64),
            next_log2(64),
            Some(2),
        );
    });
    config_local_ep(3, |regs| {
        TCU::config_recv(
            regs,
            OWN_VPE,
            rbuf2_phys,
            next_log2(64),
            next_log2(64),
            None,
        );
    });
    config_local_ep(4, |regs| {
        TCU::config_send(regs, OWN_VPE, 0x1234, PE::PE0.id(), 1, next_log2(64), 1);
    });

    let msg = Box::new(0x5678u64);
    let reply = Box::new(0x9ABCu64);

    // send message
    TCU::send(
        4,
        &*msg as *const _ as *const u8,
        util::size_of::<u64>(),
        0x1111,
        3,
    )
    .unwrap();

    // fetch message
    let rmsg = loop {
        if let Some(m) = fetch_msg(1, rbuf1_virt) {
            break m;
        }
    };
    assert_eq!({ rmsg.header.label }, 0x1234);
    assert_eq!(*rmsg.get_data::<u64>(), *msg);

    // send reply
    TCU::reply(
        1,
        &*reply as *const _ as *const u8,
        util::size_of::<u64>(),
        tcu::TCU::msg_to_offset(rbuf1_virt, rmsg),
    )
    .unwrap();

    // fetch reply
    let rmsg = loop {
        if let Some(m) = fetch_msg(3, rbuf2_virt) {
            break m;
        }
    };
    assert_eq!({ rmsg.header.label }, 0x1111);
    assert_eq!(*rmsg.get_data::<u64>(), *reply);

    // ack reply
    tcu::TCU::ack_msg(3, tcu::TCU::msg_to_offset(rbuf2_virt, rmsg)).unwrap();
}

#[no_mangle]
pub extern "C" fn env_run() {
    io::init(0, "vmtest");

    log!(crate::LOG_DEF, "Setting up paging...");
    paging::init();

    log!(crate::LOG_DEF, "Setting up interrupts...");
    STATE.set(isr::State::default());
    isr::init(STATE.get_mut());
    isr::reg(isr::TCU_ISR, tcu_irq);
    isr::reg(isr::Vector::INSTR_PAGEFAULT.val, mmu_pf);
    isr::reg(isr::Vector::LOAD_PAGEFAULT.val, mmu_pf);
    isr::reg(isr::Vector::STORE_PAGEFAULT.val, mmu_pf);
    isr::enable_irqs();

    log!(crate::LOG_DEF, "Mapping and initializing heap...");
    let heap_begin = 0xC000_0000;
    let heap_size = 0x40000;
    paging::map_anon(heap_begin, heap_size, PageFlags::RW).expect("Unable to map heap");
    unsafe {
        heap_init(heap_begin, heap_begin + heap_size);
    }

    let virt = cfg::ENV_START;
    let pte = paging::translate(virt, PageFlags::R);
    log!(
        crate::LOG_DEF,
        "Translated virt={:#x} to PTE={:#x}",
        virt,
        pte
    );

    let mut count = 0;
    for i in 1..=4 {
        TCU::invalidate_tlb();
        test_mem(i * 8);
        count += 1;
        assert_eq!(*XLATES, count);
    }

    for i in 1..=4 {
        TCU::invalidate_tlb();
        test_msgs(i * 8);
        count += 1;
        assert_eq!(*XLATES, count);
    }


    log!(crate::LOG_DEF, "Shutting down");
    exit(0);
}
