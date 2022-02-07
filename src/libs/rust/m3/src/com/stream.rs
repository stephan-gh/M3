/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use core::ops;
use core::slice;

use crate::com::{RecvGate, SendGate};
use crate::errors::{Code, Error};
use crate::mem;
use crate::serialize::{Marshallable, Sink, Source, Unmarshallable};
use crate::tcu;

/// An output stream for marshalling a TCU message and sending it via a [`SendGate`].
pub struct GateOStream<'s> {
    sink: Sink<'s>,
}

impl<'s> GateOStream<'s> {
    pub fn new(slice: &'s mut [u64]) -> Self {
        GateOStream {
            sink: Sink::new(slice),
        }
    }

    /// Returns the size of the marshalled message
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.sink.size()
    }

    /// Returns the marshalled message as a slice of words
    pub fn words(&self) -> &[u64] {
        self.sink.words()
    }

    /// Pushes the given object into the stream.
    #[inline(always)]
    pub fn push<T: Marshallable>(&mut self, item: &T) {
        item.marshall(&mut self.sink);
    }

    /// Sends the marshalled message via `gate`, using `reply_gate` for the reply.
    #[inline(always)]
    pub fn send(
        &self,
        buf: &mem::MsgBuf,
        gate: &SendGate,
        reply_gate: &RecvGate,
    ) -> Result<(), Error> {
        gate.send(buf, reply_gate)
    }

    /// Sends the marshalled message via `gate`, using `reply_gate` for the reply.
    #[inline(always)]
    pub fn call<'r>(
        &self,
        buf: &mem::MsgBuf,
        gate: &SendGate,
        reply_gate: &'r RecvGate,
    ) -> Result<GateIStream<'r>, Error> {
        gate.call(buf, reply_gate)
            .map(|m| GateIStream::new(m, reply_gate))
    }
}

/// An input stream for unmarshalling a TCU message that has been received over a [`RecvGate`].
#[derive(Debug)]
pub struct GateIStream<'r> {
    msg: &'static tcu::Message,
    source: Source<'static>,
    rgate: &'r RecvGate,
    ack: bool,
}

impl<'r> GateIStream<'r> {
    /// Creates a new `GateIStream` for `msg` that has been received over `rgate`.
    pub fn new(msg: &'static tcu::Message, rgate: &'r RecvGate) -> Self {
        // safety: we trust the TCU
        let slice = unsafe {
            #[allow(clippy::cast_ptr_alignment)]
            let ptr = msg.data.as_ptr() as *const u64;
            slice::from_raw_parts(ptr, (msg.header.length / 8) as usize)
        };

        GateIStream {
            msg,
            source: Source::new(slice),
            rgate,
            ack: true,
        }
    }

    /// Returns the receive gate this message was received with
    pub fn rgate(&self) -> &RecvGate {
        self.rgate
    }

    /// Returns the label of the message
    #[inline(always)]
    pub fn label(&self) -> tcu::Label {
        self.msg.header.label
    }

    /// Returns the size of the message
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.source.size() * mem::size_of::<u64>()
    }

    /// Returns the message
    pub fn msg(&self) -> &'static tcu::Message {
        self.msg
    }

    /// Removes the message from this gate stream, so that no ACK will be performed on drop.
    pub fn take_msg(&mut self) -> &'static tcu::Message {
        self.ack = false;
        self.msg
    }

    /// Pops an object of type `T` from the message.
    #[inline(always)]
    pub fn pop<T: Unmarshallable>(&mut self) -> Result<T, Error> {
        T::unmarshall(&mut self.source)
    }

    /// Sends `reply` as a reply to the received message.
    #[inline(always)]
    pub fn reply(&mut self, reply: &mem::MsgBuf) -> Result<(), Error> {
        match self.rgate.reply(reply, self.msg) {
            Ok(_) => {
                self.ack = false;
                Ok(())
            },
            Err(e) => Err(e),
        }
    }

    /// Sends the message marshalled by the given [`GateOStream`] as a reply on the received
    /// message.
    #[inline(always)]
    pub fn reply_os(&mut self, buf: &mem::MsgBuf) -> Result<(), Error> {
        self.reply(buf)
    }
}

impl<'r> ops::Drop for GateIStream<'r> {
    fn drop(&mut self) {
        if self.ack {
            self.rgate.ack_msg(self.msg).ok();
        }
    }
}

/// Marshalls a message from `$args` and returns the `MsgBuf`.
#[macro_export]
macro_rules! build_vmsg {
    ( $msg:expr, $( $args:expr ),* ) => ({
        // safety: we initialize these bytes below
        let mut os = unsafe { $crate::com::GateOStream::new($msg.words_mut()) };
        $( os.push(&$args); )*
        let bytes = os.size();
        // safety: we just have initialized these bytes
        unsafe { $msg.set_size(bytes) };
    });
}

/// Marshalls a message from `$args` and sends it via `$sg`, using `$rg` to receive the reply.
#[macro_export]
macro_rules! send_vmsg {
    ( $sg:expr, $rg:expr, $( $args:expr ),* ) => ({
        let mut msg = $crate::mem::MsgBuf::borrow_def();
        $crate::build_vmsg!(&mut msg, $( $args ),*);
        $sg.send(&msg, $rg)
    });
}

/// Marshalls a message from `$args` and sends it as a reply to the given [`GateIStream`].
#[macro_export]
macro_rules! reply_vmsg {
    ( $is:expr, $( $args:expr ),* ) => ({
        let mut msg = $crate::mem::MsgBuf::borrow_def();
        $crate::build_vmsg!(&mut msg, $( $args ),*);
        $is.reply_os(&msg)
    });
}

impl<'r> GateIStream<'r> {
    /// Sends the given error code as a reply.
    #[inline(always)]
    pub fn reply_error(&mut self, err: Code) -> Result<(), Error> {
        reply_vmsg!(self, err as u64)
    }
}

/// Receives a message from `rgate` and returns a [`GateIStream`] for the message.
#[inline(always)]
pub fn recv_msg(rgate: &RecvGate) -> Result<GateIStream<'_>, Error> {
    recv_reply(rgate, None)
}

/// Receives a message from `rgate` as a reply to the message that has been sent over `sgate` and
/// returns a [`GateIStream`] for the message.
#[inline(always)]
pub fn recv_reply<'r>(
    rgate: &'r RecvGate,
    sgate: Option<&SendGate>,
) -> Result<GateIStream<'r>, Error> {
    rgate.receive(sgate).map(|m| GateIStream::new(m, rgate))
}

/// Receives a message from `rgate` as a reply to the message that has been sent over `sgate` and
/// unmarshalls the result (error code). If the result is an error, it returns the error and
/// otherwise the [`GateIStream`] for the message.
#[inline(always)]
pub fn recv_result<'r>(
    rgate: &'r RecvGate,
    sgate: Option<&SendGate>,
) -> Result<GateIStream<'r>, Error> {
    let mut reply = recv_reply(rgate, sgate)?;
    let res = Code::from(reply.pop::<u32>()?);
    match res {
        Code::None => Ok(reply),
        e => Err(Error::new(e)),
    }
}

/// Marshalls a message from `$args` and sends it via `$sg`, using `$rg` to receive the reply.
/// Afterwards, it waits for the reply and returns the [`GateIStream`] for the reply.
#[macro_export]
macro_rules! send_recv {
    ( $sg:expr, $rg:expr, $( $args:expr ),* ) => ({
        let mut msg = $crate::mem::MsgBuf::borrow_def();
        $crate::build_vmsg!(&mut msg, $( $args ),*);
        $sg.call(&msg, $rg)
            .map(|m| $crate::com::GateIStream::new(m, $rg))
    });
}

/// Marshalls a message from `$args` and sends it via `$sg`, using `$rg` to receive the reply.
/// Afterwards, it waits for the reply and unmarshalls the result (error code). If the result is an
/// error, it returns the error and otherwise the [`GateIStream`] for the reply.
#[macro_export]
macro_rules! send_recv_res {
    ( $sg:expr, $rg:expr, $( $args:expr ),* ) => ({
        send_recv!($sg, $rg, $( $args ),* ).and_then(|mut reply| {
            let res = base::errors::Code::from(reply.pop::<u32>()?);
            match res {
                base::errors::Code::None => Ok(reply),
                e => Err(Error::new(e)),
            }
        })
    });
}
