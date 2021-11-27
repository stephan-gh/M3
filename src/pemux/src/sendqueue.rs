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

use base::cell::StaticRefCell;
use base::col::{DList, Vec};
use base::errors::Error;
use base::log;
use base::mem::MsgBuf;
use base::tcu;

struct SendQueue {
    queue: DList<Vec<u8>>,
}

static SQUEUE: StaticRefCell<SendQueue> = StaticRefCell::new(SendQueue {
    queue: DList::new(),
});

pub fn check_replies() {
    if let Some(msg_off) = tcu::TCU::fetch_msg(tcu::KPEX_REP) {
        log!(crate::LOG_SQUEUE, "squeue: received reply",);

        // now that we've copied the message, we can mark it read
        tcu::TCU::ack_msg(tcu::KPEX_REP, msg_off).unwrap();

        send_pending();
    }
}

pub fn send(msg: &MsgBuf) -> Result<(), Error> {
    log!(crate::LOG_SQUEUE, "squeue: trying to send msg",);

    if tcu::TCU::credits(tcu::KPEX_SEP).unwrap() > 0 {
        return do_send(msg);
    }

    log!(crate::LOG_SQUEUE, "squeue: queuing msg",);

    // copy message to heap
    let vec = msg.bytes().to_vec();
    SQUEUE.borrow_mut().queue.push_back(vec);
    Ok(())
}

fn send_pending() {
    loop {
        match SQUEUE.borrow_mut().queue.pop_front() {
            None => return,

            Some(e) => {
                log!(crate::LOG_SQUEUE, "squeue: found pending message",);

                let mut msg_buf = MsgBuf::new();
                msg_buf.set_from_slice(&e);
                if do_send(&msg_buf).is_ok() {
                    break;
                }
            },
        }
    }
}

fn do_send(msg: &MsgBuf) -> Result<(), Error> {
    log!(crate::LOG_SQUEUE, "squeue: sending msg",);
    tcu::TCU::send(tcu::KPEX_SEP, msg, 0, tcu::KPEX_REP)
}
