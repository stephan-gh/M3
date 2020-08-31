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
use base::col::{DList, Vec};
use base::errors::{Code, Error};
use base::tcu;
use thread;

use crate::ktcu;

struct Entry {
    id: u64,
    rep: tcu::EpId,
    lbl: tcu::Label,
    msg: Vec<u8>,
}

impl Entry {
    pub fn new(id: u64, rep: tcu::EpId, lbl: tcu::Label, msg: Vec<u8>) -> Self {
        Entry { id, rep, lbl, msg }
    }
}

#[derive(Eq, PartialEq)]
enum QState {
    Idle,
    Waiting,
    Aborted,
}

pub struct SendQueue {
    id: u64,
    pe: tcu::PEId,
    queue: DList<Entry>,
    cur_event: thread::Event,
    state: QState,
}

fn alloc_qid() -> u64 {
    static NEXT_ID: StaticCell<u64> = StaticCell::new(0);
    NEXT_ID.set(*NEXT_ID + 1);
    *NEXT_ID
}

fn get_event(id: u64) -> thread::Event {
    0x8000_0000_0000_0000 | id
}

impl SendQueue {
    pub fn new(id: u64, pe: tcu::PEId) -> Self {
        SendQueue {
            id,
            pe,
            queue: DList::new(),
            cur_event: 0,
            state: QState::Idle,
        }
    }

    pub fn send(
        &mut self,
        rep: tcu::EpId,
        lbl: tcu::Label,
        msg: &[u8],
    ) -> Result<thread::Event, Error> {
        klog!(SQUEUE, "SendQueue[{}]: trying to send msg", self.id);

        if self.state == QState::Aborted {
            return Err(Error::new(Code::RecvGone));
        }

        if self.state == QState::Idle {
            return self.do_send(rep, lbl, alloc_qid(), msg, msg.len());
        }

        klog!(SQUEUE, "SendQueue[{}]: queuing msg", self.id);

        let qid = alloc_qid();

        // copy message to heap
        let vec = msg.to_vec();
        self.queue.push_back(Entry::new(qid, rep, lbl, vec));
        Ok(get_event(qid))
    }

    pub fn received_reply(&mut self, msg: &'static tcu::Message) {
        klog!(SQUEUE, "SendQueue[{}]: received reply", self.id);

        assert!(self.state == QState::Waiting);
        self.state = QState::Idle;

        thread::ThreadManager::get().notify(self.cur_event, Some(msg));

        // now that we've copied the message, we can mark it read
        ktcu::ack_msg(ktcu::KSRV_EP, msg);

        self.send_pending();
    }

    pub fn abort(&mut self) {
        klog!(SQUEUE, "SendQueue[{}]: aborting", self.id);

        if self.state == QState::Waiting {
            thread::ThreadManager::get().notify(self.cur_event, None);
        }
        self.state = QState::Idle;
    }

    fn send_pending(&mut self) {
        loop {
            match self.queue.pop_front() {
                None => return,

                Some(e) => {
                    klog!(SQUEUE, "SendQueue[{}]: found pending message", self.id);

                    if self
                        .do_send(e.rep, e.lbl, e.id, &e.msg, e.msg.len())
                        .is_ok()
                    {
                        break;
                    }
                },
            }
        }
    }

    fn do_send(
        &mut self,
        rep: tcu::EpId,
        lbl: tcu::Label,
        id: u64,
        msg: &[u8],
        size: usize,
    ) -> Result<thread::Event, Error> {
        klog!(SQUEUE, "SendQueue[{}]: sending msg", self.id);

        self.cur_event = get_event(id);
        self.state = QState::Waiting;

        let rpl_lbl = self as *mut Self as tcu::Label;
        ktcu::send_to(
            self.pe,
            rep,
            lbl,
            msg.as_ptr(),
            size,
            rpl_lbl,
            ktcu::KSRV_EP,
        )?;

        Ok(self.cur_event)
    }
}

impl Drop for SendQueue {
    fn drop(&mut self) {
        self.abort();
    }
}
