/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

use base::cell::StaticRefCell;
use base::log;
use base::tmif;

use crate::activities;

use isr::{ISRArch, ISR};

#[derive(Copy, Clone)]
struct IRQCounter {
    act: activities::Id,
    counter: u64,
}

const MAX_IRQS: usize = 6;

static IRQS: StaticRefCell<[Option<IRQCounter>; MAX_IRQS]> = StaticRefCell::new([None; MAX_IRQS]);

pub fn register(act: &mut activities::ActivityRef<'_>, irq: tmif::IRQId) {
    let mut irqs = IRQS.borrow_mut();
    assert!(irqs[irq as usize].is_none());
    irqs[irq as usize] = Some(IRQCounter {
        act: act.id(),
        counter: 0,
    });
    ISR::register_ext_irq(irq);
    act.add_irq(irq);
}

pub fn wait(
    cur: &activities::ActivityRef<'_>,
    irq: Option<tmif::IRQId>,
) -> Option<activities::Event> {
    let mut irqs = IRQS.borrow_mut();
    if let Some(i) = irq {
        let cnt = &mut irqs[i as usize]?;
        if cnt.act == cur.id() && cnt.counter > 0 {
            cnt.counter -= 1;
            return Some(activities::Event::Interrupt(i));
        }
    }
    else {
        for (i, cnt) in irqs.iter_mut().flatten().enumerate() {
            if cnt.act == cur.id() && cnt.counter > 0 {
                cnt.counter -= 1;
                return Some(activities::Event::Interrupt(i as tmif::IRQId));
            }
        }
    }

    log!(crate::LOG_IRQS, "irqmask[{:#x}] enable", cur.irq_mask());
    ISR::enable_ext_irqs(cur.irq_mask());
    None
}

pub fn signal(irq: tmif::IRQId) {
    let mut irqs = IRQS.borrow_mut();
    if let Some(ref mut cnt) = irqs[irq as usize] {
        let mut act = activities::get_mut(cnt.act).unwrap();
        if !act.unblock(activities::Event::Interrupt(irq)) {
            cnt.counter += 1;
            log!(crate::LOG_IRQS, "irqs[{}] signal -> {}", irq, cnt.counter);
        }

        log!(crate::LOG_IRQS, "irqmask[{:#x}] disable", 1 << irq);
        ISR::disable_ext_irqs(1 << irq);
    }
}

pub fn remove(act: &activities::Activity) {
    if act.irq_mask() != 0 {
        let mut irqs = IRQS.borrow_mut();
        for i in 0..MAX_IRQS {
            let irq = &mut irqs[i];
            if let Some(ref cnt) = irq {
                if cnt.act == act.id() {
                    *irq = None;
                }
            }
        }

        log!(crate::LOG_IRQS, "irqmask[{:#x}] disable", act.irq_mask());
        ISR::disable_ext_irqs(act.irq_mask());
    }
}
