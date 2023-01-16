/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use base::boxed::Box;
use base::cell::{LazyStaticRefCell, StaticCell};
use base::col::VecDeque;
use base::errors::{Code, Error};
use base::mem::MsgBuf;
use base::msgqueue::{MsgQueue, MsgSender};
use base::tcu::{self, ActId, TileId};

use crate::ktcu;

pub const MAX_PENDING_MSGS: usize = 4;

static PENDING_QUEUES: LazyStaticRefCell<VecDeque<*mut SendQueue>> = LazyStaticRefCell::default();
static PENDING_MSGS: StaticCell<usize> = StaticCell::new(0);

fn delay_queue(queue: &mut SendQueue) {
    if !queue.pending {
        queue.pending = true;
        klog!(SQUEUE, "SendQueue[{:?}]: delaying", queue.id());
        PENDING_QUEUES.borrow_mut().push_back(queue as *mut _);
    }
}

fn resume_queue() {
    if let Some(q) = PENDING_QUEUES.borrow_mut().pop_front() {
        // safety: as soon as a queue is aborted/dropped, we remove it from the PENDING_QUEUES.
        // thus, whenever a queue is found here, it is still alive (and has messages pending) and
        // therefore safe to access
        unsafe {
            klog!(SQUEUE, "SendQueue[{:?}]: resuming", (*q).id());
            (*q).pending = false;
            (*q).queue.send_pending();
        }
    }
}

fn remove_queue(queue: &mut SendQueue) {
    if queue.pending {
        klog!(SQUEUE, "SendQueue[{:?}]: removing", queue.id());
        PENDING_QUEUES
            .borrow_mut()
            .retain(|q| *q != queue as *mut _);
        queue.pending = false;
    }
}

struct MetaData {
    id: u64,
    rep: tcu::EpId,
    lbl: tcu::Label,
}

#[derive(Copy, Clone, Debug)]
pub enum QueueId {
    TileMux(TileId),
    Activity(ActId),
    Serv(ActId),
}

struct KTCUSender {
    id: QueueId,
    tile: tcu::TileId,
    rpl_lbl: tcu::Label,
    cur_event: Option<thread::Event>,
}

impl MsgSender<MetaData> for KTCUSender {
    fn can_send(&self) -> bool {
        PENDING_MSGS.get() < MAX_PENDING_MSGS && self.cur_event.is_none()
    }

    fn send(&mut self, meta: MetaData, msg: &MsgBuf) -> Result<(), Error> {
        klog!(SQUEUE, "SendQueue[{:?}]: sending msg", self.id);

        ktcu::send_to(
            self.tile,
            meta.rep,
            meta.lbl,
            msg,
            self.rpl_lbl,
            ktcu::KSRV_EP,
        )?;

        self.cur_event = Some(get_event(meta.id));
        PENDING_MSGS.set(PENDING_MSGS.get() + 1);

        Ok(())
    }
}

pub struct SendQueue {
    queue: MsgQueue<KTCUSender, MetaData>,
    aborted: bool,
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
    pub fn new(id: QueueId, tile: tcu::TileId) -> Box<Self> {
        // put the queue in a box, because we use its address for identification
        let mut queue = Box::new(SendQueue {
            queue: MsgQueue::new(KTCUSender {
                id,
                tile,
                rpl_lbl: 0,
                cur_event: None,
            }),
            aborted: false,
            pending: false,
        });
        queue.queue.sender_mut().rpl_lbl = &*queue as *const Self as tcu::Label;
        queue
    }

    pub fn id(&self) -> QueueId {
        self.queue.sender().id
    }

    pub fn send(
        &mut self,
        rep: tcu::EpId,
        lbl: tcu::Label,
        msg: &MsgBuf,
    ) -> Result<thread::Event, Error> {
        klog!(SQUEUE, "SendQueue[{:?}]: trying to send msg", self.id());

        if self.aborted {
            return Err(Error::new(Code::RecvGone));
        }

        let id = alloc_qid();

        if !self.queue.send(MetaData { id, rep, lbl }, msg)? {
            klog!(SQUEUE, "SendQueue[{:?}]: queuing msg", self.id());
            if self.queue.sender().cur_event.is_none() {
                delay_queue(self);
            }
        }

        Ok(get_event(id))
    }

    pub fn receive_async(event: thread::Event) -> Result<&'static tcu::Message, Error> {
        thread::wait_for(event);
        thread::fetch_msg().ok_or_else(|| Error::new(Code::RecvGone))
    }

    pub fn received_reply(&mut self, msg: &'static tcu::Message) {
        klog!(SQUEUE, "SendQueue[{:?}]: received reply", self.id());

        if let Some(ev) = self.queue.sender_mut().cur_event.take() {
            thread::notify(ev, Some(msg));
            PENDING_MSGS.set(PENDING_MSGS.get() - 1);
        }

        // now that we've copied the message, we can mark it read
        ktcu::ack_msg(ktcu::KSRV_EP, msg);

        if !self.queue.send_pending() {
            resume_queue();
        }
    }

    pub fn abort(&mut self) {
        klog!(SQUEUE, "SendQueue[{:?}]: aborting", self.id());

        remove_queue(self);
        if let Some(ev) = self.queue.sender_mut().cur_event.take() {
            thread::notify(ev, None);
            // we were waiting for a message and won't receive it
            PENDING_MSGS.set(PENDING_MSGS.get() - 1);
            resume_queue();
        }
        self.aborted = true;
    }
}

impl Drop for SendQueue {
    fn drop(&mut self) {
        self.abort();
    }
}
