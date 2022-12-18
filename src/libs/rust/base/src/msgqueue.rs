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

//! Contains the message queue types

use crate::col::{DList, Vec};
use crate::errors::Error;
use crate::mem::MsgBuf;

struct PendingMsg<M> {
    msg: Vec<u8>,
    meta: M,
}

/// A type that can send messages
pub trait MsgSender<M> {
    /// Checks whether sending is currently possible (e.g., if the endpoint has credits)
    ///
    /// If sending is not possible, the message will be queued for a later retry.
    fn can_send(&self) -> bool;

    /// Sends the given message with meta information `meta`
    fn send(&mut self, meta: M, msg: &MsgBuf) -> Result<(), Error>;

    /// Sends the given bytes with meta information `meta`
    fn send_bytes(&mut self, meta: M, msg: &[u8]) -> Result<(), Error> {
        let mut msg_buf = MsgBuf::new();
        msg_buf.set_from_slice(msg);
        self.send(meta, &msg_buf)
    }
}

/// A simple message queue
///
/// The message queue first attempts to send a message and queues it on failures for a later retry.
/// The actual sending and testing whether sends are possible is defined by the implementation of
/// [`MsgSender`] given as the type argument `S`. Additionally, the custom meta data `M` will be
/// passed to every sent message.
pub struct MsgQueue<S: MsgSender<M>, M> {
    queue: DList<PendingMsg<M>>,
    sender: S,
}

impl<S: MsgSender<M>, M> MsgQueue<S, M> {
    /// Creates a new message queue with given sender
    pub const fn new(sender: S) -> Self {
        Self {
            queue: DList::new(),
            sender,
        }
    }

    /// Returns a reference to the sender
    pub fn sender(&self) -> &S {
        &self.sender
    }

    /// Returns a mutable reference to the sender
    pub fn sender_mut(&mut self) -> &mut S {
        &mut self.sender
    }

    /// Sends the given message with given meta data.
    ///
    /// If sending is currently possible, it happens immediately. Otherwise, the message is queued
    /// for a later retry, which needs to be triggered via [`MsgQueue::send_pending`].
    pub fn send(&mut self, meta: M, msg: &MsgBuf) -> Result<bool, Error> {
        if self.sender.can_send() {
            return self.sender.send(meta, msg).map(|_| true);
        }

        // copy message to heap
        let msg = msg.bytes().to_vec();
        self.queue.push_back(PendingMsg { msg, meta });
        Ok(false)
    }

    /// Attempts to send any queued messages
    ///
    /// Returns true if any message was sent
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
