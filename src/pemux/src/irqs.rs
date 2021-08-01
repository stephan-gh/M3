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
    vpe: Option<vpe::Id>,
    counter: u64,
}

const MAX_IRQS: usize = 5;

static IRQS: StaticCell<[IRQCounter; MAX_IRQS]> = StaticCell::new(
    [IRQCounter {
        vpe: None,
        counter: 0,
    }; MAX_IRQS],
);

pub fn wait(vpe: vpe::Id, irqs: u32, timeout_ns: Option<u64>) {
    for i in 0..MAX_IRQS {
        if (irqs & (1 << i)) != 0 {
            let cnt = &mut IRQS.get_mut()[i];
            assert!(cnt.vpe.is_none());
            if cnt.counter == 0 {
                cnt.vpe = Some(vpe);
                let tcu_irq = tcu::IRQ::from(i as u64);
                log!(crate::LOG_IRQS, "irqs[{}] enabling", tcu_irq);
                isr::enable_irq(tcu_irq);
            }
            else {
                cnt.counter -= 1;
                log!(crate::LOG_IRQS, "irqs[{}] fetch -> {}", i, cnt.counter);
                remove(vpe, irqs);
                return;
            }
        }
    }

    vpe::cur().block(None, Some(vpe::Event::Interrupt(irqs)), timeout_ns);
}

#[cfg(target_vendor = "hw")]
pub fn signal(irq: tcu::IRQ) {
    let cnt = &mut IRQS.get_mut()[irq.val as usize];
    if let Some(id) = cnt.vpe {
        let vpe = vpe::get_mut(id).unwrap();
        if let Some(vpe::Event::Interrupt(irqs)) = vpe.wait_event {
            remove(id, irqs);
        }
        vpe.unblock(Some(vpe::Event::Interrupt(0)), false);
    }
    else {
        cnt.counter += 1;
        log!(crate::LOG_IRQS, "irqs[{}] signal -> {}", irq, cnt.counter);
    }

    log!(crate::LOG_IRQS, "irqs[{}] disable", irq);
    isr::disable_irq(irq);
}

pub fn remove(vpe: vpe::Id, irqs: u32) {
    for i in 0..MAX_IRQS {
        if (irqs & (1 << i)) != 0 {
            let cnt = &mut IRQS.get_mut()[i];
            if cnt.vpe.is_some() {
                assert_eq!(cnt.vpe, Some(vpe));
                cnt.vpe = None;
            }
        }
    }
}
