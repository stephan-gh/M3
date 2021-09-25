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

use base::cell::{LazyStaticCell, StaticCell};
use base::col::{DList, Vec, VecDeque};
use base::errors::{Code, Error};
use base::mem::MsgBuf;
use base::tcu::{self, PEId, VPEId};

use crate::ktcu;

pub const MAX_PENDING_MSGS: usize = 4;

static PENDING_QUEUES: LazyStaticCell<VecDeque<*mut SendQueue>> = LazyStaticCell::default();
static PENDING_MSGS: StaticCell<usize> = StaticCell::new(0);

fn delay_queue(queue: &mut SendQueue) {
    if !queue.pending {
        queue.pending = true;
        klog!(SQUEUE, "SendQueue[{:?}]: delaying", queue.id);
        PENDING_QUEUES.get_mut().push_back(queue as *mut _);
    }
}

fn resume_queue() {
    if let Some(q) = PENDING_QUEUES.get_mut().pop_front() {
        // safety: as soon as a queue is aborted/dropped, we remove it from the PENDING_QUEUES.
        // thus, whenever a queue is found here, it is still alive (and has messages pending) and
        // therefore safe to access
        unsafe {
            klog!(SQUEUE, "SendQueue[{:?}]: resuming", (*q).id);
            (*q).pending = false;
            (*q).send_pending();
        }
    }
}

fn remove_queue(queue: &mut SendQueue) {
    if queue.pending {
        klog!(SQUEUE, "SendQueue[{:?}]: removing", queue.id);
        PENDING_QUEUES.get_mut().retain(|q| *q != queue as *mut _);
        queue.pending = false;
    }
}

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

#[derive(Eq, PartialEq, Debug)]
enum QState {
    Idle,
    Waiting,
    Aborted,
}

#[derive(Debug)]
pub enum QueueId {
    #[allow(dead_code)]
    PEMux(PEId),
    VPE(VPEId),
    Serv(VPEId),
}

pub struct SendQueue {
    id: QueueId,
    pe: tcu::PEId,
    queue: DList<Entry>,
    cur_event: thread::Event,
    state: QState,
    pending: bool,
}

pub fn init_queues() {
    PENDING_QUEUES.set(VecDeque::new());
}

fn alloc_qid() -> u64 {
    static NEXT_ID: StaticCell<u64> = StaticCell::new(0);
    NEXT_ID.set(NEXT_ID.get() + 1);
    NEXT_ID.get()
}

fn get_event(id: u64) -> thread::Event {
    0x8000_0000_0000_0000 | id
}

impl SendQueue {
    pub fn new(id: QueueId, pe: tcu::PEId) -> Self {
        SendQueue {
            id,
            pe,
            queue: DList::new(),
            cur_event: 0,
            state: QState::Idle,
            pending: false,
        }
    }

    pub fn send(
        &mut self,
        rep: tcu::EpId,
        lbl: tcu::Label,
        msg: &MsgBuf,
    ) -> Result<thread::Event, Error> {
        klog!(SQUEUE, "SendQueue[{:?}]: trying to send msg", self.id);

        if self.state == QState::Aborted {
            return Err(Error::new(Code::RecvGone));
        }

        let qid = alloc_qid();

        if PENDING_MSGS.get() < MAX_PENDING_MSGS && self.state == QState::Idle {
            return self.do_send(rep, lbl, qid, msg);
        }

        klog!(SQUEUE, "SendQueue[{:?}]: queuing msg", self.id);

        // copy message to heap
        let vec = msg.bytes().to_vec();
        self.queue.push_back(Entry::new(qid, rep, lbl, vec));
        if self.state == QState::Idle {
            delay_queue(self);
        }
        Ok(get_event(qid))
    }

    pub fn receive_async(event: thread::Event) -> Result<&'static tcu::Message, Error> {
        thread::ThreadManager::get().wait_for(event);
        thread::ThreadManager::get()
            .fetch_msg()
            .ok_or_else(|| Error::new(Code::RecvGone))
    }

    pub fn received_reply(&mut self, msg: &'static tcu::Message) {
        klog!(SQUEUE, "SendQueue[{:?}]: received reply", self.id);

        // ignore the message if we we're not waiting
        if self.state != QState::Waiting {
            return;
        }

        self.state = QState::Idle;

        thread::ThreadManager::get().notify(self.cur_event, Some(msg));

        // now that we've copied the message, we can mark it read
        ktcu::ack_msg(ktcu::KSRV_EP, msg);

        PENDING_MSGS.set(PENDING_MSGS.get() - 1);

        if self.queue.is_empty() {
            resume_queue();
        }
        else {
            self.send_pending();
        }
    }

    pub fn abort(&mut self) {
        klog!(SQUEUE, "SendQueue[{:?}]: aborting", self.id);

        remove_queue(self);
        if self.state == QState::Waiting {
            thread::ThreadManager::get().notify(self.cur_event, None);
            // we were waiting for a message and won't receive it
            PENDING_MSGS.set(PENDING_MSGS.get() - 1);
            resume_queue();
        }
        self.state = QState::Idle;
    }

    fn send_pending(&mut self) {
        loop {
            match self.queue.pop_front() {
                None => return,

                Some(e) => {
                    klog!(SQUEUE, "SendQueue[{:?}]: found pending message", self.id);

                    let mut msg_buf = MsgBuf::new();
                    msg_buf.set_from_slice(&e.msg);
                    if self.do_send(e.rep, e.lbl, e.id, &msg_buf).is_ok() {
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
        msg: &MsgBuf,
    ) -> Result<thread::Event, Error> {
        klog!(SQUEUE, "SendQueue[{:?}]: sending msg", self.id);

        self.cur_event = get_event(id);
        self.state = QState::Waiting;

        let rpl_lbl = self as *mut Self as tcu::Label;
        ktcu::send_to(self.pe, rep, lbl, msg, rpl_lbl, ktcu::KSRV_EP)?;

        PENDING_MSGS.set(PENDING_MSGS.get() + 1);

        Ok(self.cur_event)
    }
}

impl Drop for SendQueue {
    fn drop(&mut self) {
        self.abort();
    }
}
