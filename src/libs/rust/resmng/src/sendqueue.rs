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

use m3::cell::StaticCell;
use m3::col::{DList, String, Vec};
use m3::com::{RGateArgs, RecvGate, SendGate};
use m3::dtu;
use m3::errors::Error;
use thread;

use childs::Id;
use services;

struct Entry {
    id: u64,
    msg: Vec<u8>,
}

impl Entry {
    pub fn new(id: u64, msg: Vec<u8>) -> Self {
        Entry { id, msg }
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

static RGATE: StaticCell<Option<RecvGate>> = StaticCell::new(None);

fn alloc_qid() -> u64 {
    static NEXT_ID: StaticCell<u64> = StaticCell::new(0);
    NEXT_ID.set(*NEXT_ID + 1);
    *NEXT_ID
}

fn get_event(id: u64) -> thread::Event {
    0x8000_0000_0000_0000 | id
}

pub fn init() {
    let mut rgate = RecvGate::new_with(RGateArgs::default().order(11).msg_order(6))
        .expect("Unable to create service rgate");
    rgate.activate().expect("Unable to activate service rgate");

    RGATE.set(Some(rgate));
}

pub fn check_replies() {
    let rg = RGATE.get().as_ref().unwrap();

    if let Some(msg) = dtu::DTUIf::fetch_msg(rg) {
        if let Ok(serv) = services::get().get_by_id(msg.header.label as Id) {
            serv.queue().received_reply(rg, msg);
        }
        else {
            dtu::DTUIf::mark_read(rg, msg);
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

    pub fn send(&mut self, msg: &[u8]) -> Result<thread::Event, Error> {
        log!(
            crate::LOG_SQUEUE,
            "{}:squeue: trying to send msg",
            self.serv_name()
        );

        if self.state == QState::Idle {
            return self.do_send(alloc_qid(), msg);
        }

        log!(crate::LOG_SQUEUE, "{}:squeue: queuing msg", self.serv_name());

        let qid = alloc_qid();

        // copy message to heap
        let vec = msg.to_vec();
        self.queue.push_back(Entry::new(qid, vec));
        Ok(get_event(qid))
    }

    fn serv_name(&self) -> &String {
        services::get().get_by_id(self.sid).unwrap().name()
    }

    fn received_reply(&mut self, rg: &RecvGate, msg: &'static dtu::Message) {
        log!(crate::LOG_SQUEUE, "{}:squeue: received reply", self.serv_name());

        assert!(self.state == QState::Waiting);
        self.state = QState::Idle;

        thread::ThreadManager::get().notify(self.cur_event, Some(msg));

        // now that we've copied the message, we can mark it read
        dtu::DTUIf::mark_read(rg, msg);

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
                        self.serv_name()
                    );

                    if self.do_send(e.id, &e.msg).is_ok() {
                        break;
                    }
                },
            }
        }
    }

    fn do_send(&mut self, id: u64, msg: &[u8]) -> Result<thread::Event, Error> {
        log!(crate::LOG_SQUEUE, "{}:squeue: sending msg", self.serv_name());

        self.cur_event = get_event(id);
        self.state = QState::Waiting;

        let rgate = &RGATE.get().as_ref().unwrap();
        self.sgate
            .send_with_rlabel(msg, rgate, dtu::Label::from(self.sid))?;

        Ok(self.cur_event)
    }
}

impl Drop for SendQueue {
    fn drop(&mut self) {
        if self.state == QState::Waiting {
            thread::ThreadManager::get().notify(self.cur_event, None);
        }

        while !self.queue.is_empty() {
            self.queue.pop_front();
        }
    }
}
