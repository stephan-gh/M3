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
use base::log;
use base::pexif;

use crate::vpe;

#[derive(Copy, Clone)]
struct IRQCounter {
    vpe: vpe::Id,
    counter: u64,
}

const MAX_IRQS: usize = 5;

static IRQS: StaticCell<[Option<IRQCounter>; MAX_IRQS]> = StaticCell::new([None; MAX_IRQS]);

pub fn register(vpe: &mut vpe::VPE, irq: pexif::IRQId) {
    assert!(IRQS[irq as usize].is_none());
    IRQS.get_mut()[irq as usize] = Some(IRQCounter {
        vpe: vpe.id(),
        counter: 0,
    });
    vpe.add_irq(irq);
}

pub fn wait(cur: &vpe::VPE, irq: Option<pexif::IRQId>) -> Option<vpe::Event> {
    if let Some(i) = irq {
        let cnt = &mut IRQS.get_mut()[i as usize]?;
        if cnt.vpe == cur.id() && cnt.counter > 0 {
            cnt.counter -= 1;
            return Some(vpe::Event::Interrupt(i));
        }
    }
    else {
        for (i, cnt) in IRQS.get_mut().iter_mut().flatten().enumerate() {
            if cnt.vpe == cur.id() {
                if cnt.counter > 0 {
                    cnt.counter -= 1;
                    return Some(vpe::Event::Interrupt(i as pexif::IRQId));
                }
            }
        }
    }

    log!(crate::LOG_IRQS, "irqmask[{:#x}] enable", cur.irq_mask());
    isr::enable_ext_irqs(cur.irq_mask());
    None
}

pub fn signal(irq: pexif::IRQId) {
    if let Some(ref mut cnt) = IRQS.get_mut()[irq as usize] {
        let vpe = vpe::get_mut(cnt.vpe).unwrap();
        if !vpe.unblock(vpe::Event::Interrupt(irq)) {
            cnt.counter += 1;
            log!(crate::LOG_IRQS, "irqs[{}] signal -> {}", irq, cnt.counter);
        }

        log!(crate::LOG_IRQS, "irqmask[{:#x}] disable", 1 << irq);
        isr::disable_ext_irqs(1 << irq);
    }
}

pub fn remove(vpe: &vpe::VPE) {
    if vpe.irq_mask() != 0 {
        for i in 0..MAX_IRQS {
            let irq = &mut IRQS.get_mut()[i];
            if let Some(ref cnt) = irq {
                if cnt.vpe == vpe.id() {
                    *irq = None;
                }
            }
        }

        log!(crate::LOG_IRQS, "irqmask[{:#x}] disable", vpe.irq_mask());
        isr::disable_ext_irqs(vpe.irq_mask());
    }
}
