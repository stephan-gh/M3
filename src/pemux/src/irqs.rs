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
use base::tcu;

use crate::vpe;

#[derive(Copy, Clone)]
struct IRQCounter {
    vpe: vpe::Id,
    counter: u64,
}

const MAX_IRQS: usize = 5;

static IRQS: StaticCell<[Option<IRQCounter>; MAX_IRQS]> = StaticCell::new([None; MAX_IRQS]);

pub fn register(vpe: vpe::Id, irq: tcu::IRQ) {
    assert!(IRQS[irq.val as usize].is_none());
    IRQS.get_mut()[irq.val as usize] = Some(IRQCounter { vpe, counter: 0 });
    vpe::get_mut(vpe)
        .unwrap()
        .add_irq(isr::to_plic_irq(irq).unwrap());
}

pub fn wait(cur: &vpe::VPE, irq: Option<tcu::IRQ>) -> Option<vpe::Event> {
    if let Some(i) = irq {
        let cnt = &mut IRQS.get_mut()[i.val as usize]?;
        if cnt.vpe == cur.id() && cnt.counter > 0 {
            cnt.counter -= 1;
            return Some(vpe::Event::Interrupt(tcu::IRQ::from(i.val as u64)));
        }
    }
    else {
        for (i, cnt) in IRQS.get_mut().iter_mut().flatten().enumerate() {
            if cnt.vpe == cur.id() {
                if cnt.counter > 0 {
                    cnt.counter -= 1;
                    return Some(vpe::Event::Interrupt(tcu::IRQ::from(i as u64)));
                }
            }
        }
    }

    log!(crate::LOG_IRQS, "irqmask[{:#x}] enable", cur.irq_mask());
    isr::enable_irq_mask(cur.irq_mask());
    None
}

#[cfg(target_vendor = "hw")]
pub fn signal(irq: tcu::IRQ) {
    if let Some(ref mut cnt) = IRQS.get_mut()[irq.val as usize] {
        let vpe = vpe::get_mut(cnt.vpe).unwrap();
        if !vpe.unblock(vpe::Event::Interrupt(irq)) {
            cnt.counter += 1;
            log!(crate::LOG_IRQS, "irqs[{}] signal -> {}", irq, cnt.counter);
        }

        log!(crate::LOG_IRQS, "irqs[{}] disable", irq);
        isr::disable_irq(irq);
    }
}

pub fn remove(vpe: vpe::Id) {
    for i in 0..MAX_IRQS {
        let irq = &mut IRQS.get_mut()[i];
        if let Some(ref cnt) = irq {
            if cnt.vpe == vpe {
                *irq = None;
                let tcu_irq = tcu::IRQ::from(i as u64);
                log!(crate::LOG_IRQS, "irqs[{}] disable", tcu_irq);
                isr::disable_irq(tcu_irq);
            }
        }
    }
}
