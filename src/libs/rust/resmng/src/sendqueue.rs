/*
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

use m3::cap::Selector;
use m3::cell::LazyStaticRefCell;
use m3::com::{MsgQueue, MsgSender, RecvGate, SendGate};
use m3::errors::Error;
use m3::log;
use m3::mem::MsgBuf;
use m3::server::DEF_MAX_CLIENTS;
use m3::tcu;

use crate::childs::Id;
use crate::events;
use crate::resources::Resources;

pub const RBUF_MSG_SIZE: usize = 1 << 6;
pub const RBUF_SIZE: usize = RBUF_MSG_SIZE * DEF_MAX_CLIENTS;

struct GateSender {
    sid: Id,
    sgate: SendGate,
    cur_event: Option<thread::Event>,
}

impl MsgSender<thread::Event> for GateSender {
    fn can_send(&self) -> bool {
        self.cur_event.is_none()
    }

    fn send(&mut self, event: thread::Event, msg: &MsgBuf) -> Result<(), Error> {
        log!(crate::LOG_SQUEUE, "{}:squeue: sending msg", self.sid);

        // we need the conversion, because the size of label is target dependent
        self.sgate
            .send_with_rlabel(msg, &RGATE.borrow(), tcu::Label::from(self.sid))
            .map(|_| {
                self.cur_event = Some(event);
            })
    }
}

static RGATE: LazyStaticRefCell<RecvGate> = LazyStaticRefCell::default();

pub fn init(rgate: RecvGate) {
    RGATE.set(rgate);
}

pub fn check_replies(res: &mut Resources) {
    let rgate = RGATE.borrow();
    if let Ok(msg) = rgate.fetch() {
        if let Ok(serv) = res.services_mut().get_mut_by_id(msg.header.label() as Id) {
            serv.queue().received_reply(&rgate, msg);
        }
        else {
            rgate.ack_msg(msg).unwrap();
        }
    }
}

pub struct SendQueue {
    queue: MsgQueue<GateSender, thread::Event>,
}

impl SendQueue {
    pub fn new(sid: Id, sgate: SendGate) -> Self {
        SendQueue {
            queue: MsgQueue::new(GateSender {
                sid,
                sgate,
                cur_event: None,
            }),
        }
    }

    pub fn sid(&self) -> Id {
        self.queue.sender().sid
    }

    pub fn sgate_sel(&self) -> Selector {
        self.queue.sender().sgate.sel()
    }

    pub fn send(&mut self, msg: &MsgBuf) -> Result<thread::Event, Error> {
        let event = events::alloc_event();
        if !self.queue.send(event, msg)? {
            log!(crate::LOG_SQUEUE, "{}:squeue: queuing msg", self.sid());
        }
        Ok(event)
    }

    fn received_reply(&mut self, rg: &RecvGate, msg: &'static tcu::Message) {
        log!(crate::LOG_SQUEUE, "{}:squeue: received reply", self.sid());

        let event = self.queue.sender_mut().cur_event.take().unwrap();
        thread::notify(event, Some(msg));

        // now that we've copied the message, we can mark it read
        rg.ack_msg(msg).unwrap();

        self.queue.send_pending();
    }
}

impl Drop for SendQueue {
    fn drop(&mut self) {
        if let Some(ev) = self.queue.sender_mut().cur_event.take() {
            thread::notify(ev, None);
        }
    }
}
