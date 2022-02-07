/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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

use crate::col::{DList, Vec};
use crate::errors::Error;
use crate::mem::MsgBuf;

struct PendingMsg<M> {
    msg: Vec<u8>,
    meta: M,
}

pub trait MsgSender<M> {
    fn can_send(&self) -> bool;
    fn send(&mut self, meta: M, msg: &MsgBuf) -> Result<(), Error>;
    fn send_bytes(&mut self, meta: M, msg: &[u8]) -> Result<(), Error> {
        let mut msg_buf = MsgBuf::new();
        msg_buf.set_from_slice(msg);
        self.send(meta, &msg_buf)
    }
}

pub struct MsgQueue<S: MsgSender<M>, M> {
    queue: DList<PendingMsg<M>>,
    sender: S,
}

impl<S: MsgSender<M>, M> MsgQueue<S, M> {
    pub const fn new(sender: S) -> Self {
        Self {
            queue: DList::new(),
            sender,
        }
    }

    pub fn sender(&self) -> &S {
        &self.sender
    }

    pub fn sender_mut(&mut self) -> &mut S {
        &mut self.sender
    }

    pub fn send(&mut self, meta: M, msg: &MsgBuf) -> Result<bool, Error> {
        if self.sender.can_send() {
            return self.sender.send(meta, msg).map(|_| true);
        }

        // copy message to heap
        let msg = msg.bytes().to_vec();
        self.queue.push_back(PendingMsg { msg, meta });
        Ok(false)
    }

    pub fn send_pending(&mut self) -> bool {
        loop {
            match self.queue.pop_front() {
                None => break false,

                Some(e) => {
                    if self.sender.send_bytes(e.meta, &e.msg).is_ok() {
                        break true;
                    }
                },
            }
        }
    }
}
