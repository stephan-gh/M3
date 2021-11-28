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

use m3::cap::Selector;
use m3::cell::LazyStaticRefCell;
use m3::col::{DList, Vec};
use m3::com::{RecvGate, SendGate};
use m3::errors::Error;
use m3::log;
use m3::mem::MsgBuf;
use m3::server::DEF_MAX_CLIENTS;
use m3::tcu;

use crate::childs::Id;
use crate::events;
use crate::services;

pub const RBUF_MSG_SIZE: usize = 1 << 6;
pub const RBUF_SIZE: usize = RBUF_MSG_SIZE * DEF_MAX_CLIENTS;

struct Entry {
    event: thread::Event,
    msg: Vec<u8>,
}

impl Entry {
    pub fn new(event: thread::Event, msg: Vec<u8>) -> Self {
        Entry { event, msg }
    }
}

#[derive(Eq, PartialEq)]
enum QState {
    Idle,
    Waiting,
}

pub struct SendQueue {
    sid: Id,
    sgate: SendGate,
    queue: DList<Entry>,
    cur_event: thread::Event,
    state: QState,
}

static RGATE: LazyStaticRefCell<RecvGate> = LazyStaticRefCell::default();

pub fn init(rgate: RecvGate) {
    RGATE.set(rgate);
}

pub fn check_replies() {
    let rgate = RGATE.borrow();
    if let Some(msg) = rgate.fetch() {
        if let Ok(mut serv) = services::get_mut_by_id(msg.header.label as Id) {
            serv.queue().received_reply(&rgate, msg);
        }
        else {
            rgate.ack_msg(msg).unwrap();
        }
    }
}

impl SendQueue {
    pub fn new(sid: Id, sgate: SendGate) -> Self {
        SendQueue {
            sid,
            sgate,
            queue: DList::new(),
            cur_event: 0,
            state: QState::Idle,
        }
    }

    pub fn sgate_sel(&self) -> Selector {
        self.sgate.sel()
    }

    pub fn send(&mut self, msg: &MsgBuf) -> Result<thread::Event, Error> {
        log!(crate::LOG_SQUEUE, "{}:squeue: trying to send msg", self.sid);

        let event = events::alloc_event();

        if self.state == QState::Idle {
            return self.do_send(event, msg);
        }

        log!(crate::LOG_SQUEUE, "{}:squeue: queuing msg", self.sid);

        // copy message to heap
        let vec = msg.bytes().to_vec();
        self.queue.push_back(Entry::new(event, vec));
        Ok(event)
    }

    fn received_reply(&mut self, rg: &RecvGate, msg: &'static tcu::Message) {
        log!(crate::LOG_SQUEUE, "{}:squeue: received reply", self.sid);

        assert!(self.state == QState::Waiting);
        self.state = QState::Idle;

        thread::notify(self.cur_event, Some(msg));

        // now that we've copied the message, we can mark it read
        rg.ack_msg(msg).unwrap();

        self.send_pending();
    }

    fn send_pending(&mut self) {
        loop {
            match self.queue.pop_front() {
                None => return,

                Some(e) => {
                    log!(
                        crate::LOG_SQUEUE,
                        "{}:squeue: found pending message",
                        self.sid
                    );

                    let mut msg_buf = MsgBuf::new();
                    msg_buf.set_from_slice(&e.msg);
                    if self.do_send(e.event, &msg_buf).is_ok() {
                        break;
                    }
                },
            }
        }
    }

    fn do_send(&mut self, event: thread::Event, msg: &MsgBuf) -> Result<thread::Event, Error> {
        log!(crate::LOG_SQUEUE, "{}:squeue: sending msg", self.sid);

        self.cur_event = event;
        self.state = QState::Waiting;

        #[allow(clippy::useless_conversion)]
        self.sgate
            .send_with_rlabel(msg, &RGATE.borrow(), tcu::Label::from(self.sid))?;

        Ok(self.cur_event)
    }
}

impl Drop for SendQueue {
    fn drop(&mut self) {
        if self.state == QState::Waiting {
            thread::notify(self.cur_event, None);
        }

        while !self.queue.is_empty() {
            self.queue.pop_front();
        }
    }
}
