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

const MAX_IRQS: usize = 8;

static IRQS: StaticCell<[IRQCounter; MAX_IRQS]> = StaticCell::new(
    [IRQCounter {
        vpe: None,
        counter: 0,
    }; 8],
);

pub fn wait(vpe: vpe::Id, irq: tcu::IRQ) {
    let cnt = &mut IRQS.get_mut()[irq.val as usize];
    assert!(cnt.vpe.is_none());
    if cnt.counter == 0 {
        cnt.vpe = Some(vpe);
        vpe::cur().block(None, Some(vpe::Event::Interrupt(irq)), None);
    }
    else {
        cnt.counter -= 1;
        log!(crate::LOG_IRQS, "irqs[{}] fetch -> {}", irq, cnt.counter);
    }
}

#[cfg(target_vendor = "hw")]
pub fn signal(irq: tcu::IRQ) {
    let cnt = &mut IRQS.get_mut()[irq.val as usize];
    if let Some(id) = cnt.vpe {
        cnt.vpe = None;
        vpe::get_mut(id)
            .unwrap()
            .unblock(Some(vpe::Event::Interrupt(irq)), false);
    }
    else {
        cnt.counter += 1;
        log!(crate::LOG_IRQS, "irqs[{}] signal -> {}", irq, cnt.counter);
    }
}

pub fn remove(vpe: vpe::Id, irq: tcu::IRQ) {
    let cnt = &mut IRQS.get_mut()[irq.val as usize];
    assert_eq!(cnt.vpe, Some(vpe));
    cnt.vpe = None;
}
