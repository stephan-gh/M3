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

use col::{String, Vec};
use com::{RecvGate, SendGate};
use core::intrinsics;
use core::mem::MaybeUninit;
use core::ops;
use core::slice;
use dtu;
use errors::{Code, Error};
use libc;
use mem::heap;
use serialize::{Marshallable, Sink, Source, Unmarshallable};
use util;

const MAX_MSG_SIZE: usize = 512;

/// A sink for marshalling that uses a static array internally.
pub struct ArraySink {
    arr: [u64; MAX_MSG_SIZE / 8],
    pos: usize,
}

impl Default for ArraySink {
    fn default() -> Self {
        #[allow(clippy::uninit_assumed_init)]
        ArraySink {
            arr: unsafe { MaybeUninit::uninit().assume_init() },
            pos: 0,
        }
    }
}

impl Sink for ArraySink {
    #[inline(always)]
    fn size(&self) -> usize {
        self.pos * util::size_of::<u64>()
    }

    #[inline(always)]
    fn words(&self) -> &[u64] {
        &self.arr[0..self.pos]
    }

    #[inline(always)]
    fn push(&mut self, item: &dyn Marshallable) {
        item.marshall(self);
    }

    #[inline(always)]
    fn push_word(&mut self, word: u64) {
        self.arr[self.pos] = word;
        self.pos += 1;
    }

    fn push_str(&mut self, b: &str) {
        let len = b.len() + 1;
        self.push_word(len as u64);

        copy_from_str(&mut self.arr[self.pos..], b);
        self.pos += (len + 7) / 8;
    }
}

/// A sink for marshalling that uses a [`Vec`] internally.
pub struct VecSink {
    vec: Vec<u64>,
}

impl Default for VecSink {
    fn default() -> Self {
        VecSink { vec: Vec::new() }
    }
}

impl Sink for VecSink {
    fn size(&self) -> usize {
        self.vec.len() * util::size_of::<u64>()
    }

    fn words(&self) -> &[u64] {
        &self.vec
    }

    fn push(&mut self, item: &dyn Marshallable) {
        item.marshall(self);
    }

    fn push_word(&mut self, word: u64) {
        self.vec.push(word);
    }

    fn push_str(&mut self, b: &str) {
        let len = b.len() + 1;
        self.push_word(len as u64);

        let elems = (len + 7) / 8;
        let cur = self.vec.len();
        self.vec.reserve_exact(elems);

        unsafe {
            self.vec.set_len(cur + elems);
            copy_from_str(&mut self.vec.as_mut_slice()[cur..cur + elems], b);
        }
    }
}

/// A source for unmarshalling that uses a DTU message internally
#[derive(Debug)]
pub struct MsgSource {
    msg: &'static dtu::Message,
    pos: usize,
}

impl MsgSource {
    /// Creates a new `MsgSource` for given DTU message.
    pub fn new(msg: &'static dtu::Message) -> Self {
        MsgSource { msg, pos: 0 }
    }

    /// Returns a slice to the message data.
    #[inline(always)]
    pub fn data(&self) -> &'static [u64] {
        unsafe {
            #[allow(clippy::cast_ptr_alignment)]
            let ptr = self.msg.data.as_ptr() as *const u64;
            slice::from_raw_parts(ptr, (self.msg.header.length / 8) as usize)
        }
    }
}

fn copy_from_str(words: &mut [u64], s: &str) {
    unsafe {
        let addr = words.as_mut_ptr() as usize;
        libc::memcpy(
            addr as *mut libc::c_void,
            s.as_bytes().as_ptr() as *const libc::c_void,
            s.len(),
        );
        // null termination
        let end: &mut u8 = intrinsics::transmute(addr + s.len());
        *end = 0u8;
    }
}

fn copy_str_from(s: &[u64], len: usize) -> String {
    unsafe {
        let bytes: *mut libc::c_void = s.as_ptr() as *mut libc::c_void;
        let copy = heap::alloc(len + 1);
        libc::memcpy(copy, bytes, len);
        String::from_raw_parts(copy as *mut u8, len, len)
    }
}

fn str_slice_from(s: &[u64], len: usize) -> &'static str {
    unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(s.as_ptr() as *const u8, len))
    }
}

impl Source for MsgSource {
    #[inline(always)]
    fn pop_word(&mut self) -> u64 {
        self.pos += 1;
        self.data()[self.pos - 1]
    }

    fn pop_str(&mut self) -> String {
        let len = self.pop_word() as usize;
        let res = copy_str_from(&self.data()[self.pos..], len - 1);
        self.pos += (len + 7) / 8;
        res
    }

    fn pop_str_slice(&mut self) -> &'static str {
        let len = self.pop_word() as usize;
        let str = str_slice_from(&self.data()[self.pos..], len - 1);
        self.pos += (len + 7) / 8;
        str
    }
}

/// A source for unmarshalling that uses a slice internally.
pub struct SliceSource<'s> {
    slice: &'s [u64],
    pos: usize,
}

impl<'s> SliceSource<'s> {
    /// Creates a new `SliceSource` for given slice.
    pub fn new(s: &'s [u64]) -> SliceSource<'s> {
        SliceSource { slice: s, pos: 0 }
    }

    /// Pops an object of type `T` from the source.
    pub fn pop<T: Unmarshallable>(&mut self) -> T {
        T::unmarshall(self)
    }
}

impl<'s> Source for SliceSource<'s> {
    fn pop_word(&mut self) -> u64 {
        self.pos += 1;
        self.slice[self.pos - 1]
    }

    fn pop_str(&mut self) -> String {
        let len = self.pop_word() as usize;
        let res = copy_str_from(&self.slice[self.pos..], len - 1);
        self.pos += (len + 7) / 8;
        res
    }

    fn pop_str_slice(&mut self) -> &'static str {
        let len = self.pop_word() as usize;
        let str = str_slice_from(&self.slice[self.pos..], len - 1);
        self.pos += (len + 7) / 8;
        str
    }
}

/// An output stream for marshalling a DTU message and sending it via a [`SendGate`].
pub struct GateOStream {
    buf: ArraySink,
}

impl Default for GateOStream {
    fn default() -> Self {
        GateOStream {
            buf: ArraySink::default(),
        }
    }
}

impl GateOStream {
    /// Returns the size of the marshalled message
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.buf.size()
    }

    /// Returns the marshalled message as a slice of words
    pub fn words(&self) -> &[u64] {
        self.buf.words()
    }

    /// Pushes the given object into the stream.
    #[inline(always)]
    pub fn push<T: Marshallable>(&mut self, item: &T) {
        item.marshall(&mut self.buf);
    }

    /// Sends the marshalled message via `gate`, using `reply_gate` for the reply.
    #[inline(always)]
    pub fn send(&self, gate: &SendGate, reply_gate: &RecvGate) -> Result<(), Error> {
        gate.send(self.buf.words(), reply_gate)
    }

    /// Sends the marshalled message via `gate`, using `reply_gate` for the reply.
    #[inline(always)]
    pub fn call<'r>(
        &self,
        gate: &SendGate,
        reply_gate: &'r RecvGate,
    ) -> Result<GateIStream<'r>, Error> {
        gate.call(self.buf.words(), reply_gate)
    }
}

/// An input stream for unmarshalling a DTU message that has been received over a [`RecvGate`].
#[derive(Debug)]
pub struct GateIStream<'r> {
    source: MsgSource,
    rgate: &'r RecvGate,
    ack: bool,
}

impl<'r> GateIStream<'r> {
    /// Creates a new `GateIStream` for `msg` that has been received over `rgate`.
    pub fn new(msg: &'static dtu::Message, rgate: &'r RecvGate) -> Self {
        GateIStream {
            source: MsgSource::new(msg),
            rgate,
            ack: true,
        }
    }

    /// Returns the label of the message
    #[inline(always)]
    pub fn label(&self) -> dtu::Label {
        self.source.msg.header.label
    }

    /// Returns the size of the message
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.source.data().len() * util::size_of::<u64>()
    }

    /// Removes the message from this gate stream, so that no ACK will be performed on drop.
    pub fn take_msg(&mut self) -> &'static dtu::Message {
        self.ack = false;
        self.source.msg
    }

    /// Pops an object of type `T` from the message.
    #[inline(always)]
    pub fn pop<T: Unmarshallable>(&mut self) -> T {
        T::unmarshall(&mut self.source)
    }

    /// Sends `reply` as a reply to the received message.
    #[inline(always)]
    pub fn reply<T>(&mut self, reply: &[T]) -> Result<(), Error> {
        match self.rgate.reply(reply, self.source.msg) {
            Ok(_) => {
                self.ack = false;
                Ok(())
            },
            Err(e) => Err(e),
        }
    }

    /// Sends the message marshalled by the given `GateOStream` as a reply on the received message.
    #[inline(always)]
    pub fn reply_os(&mut self, os: &GateOStream) -> Result<(), Error> {
        self.reply(os.buf.words())
    }
}

impl<'r> ops::Drop for GateIStream<'r> {
    fn drop(&mut self) {
        if self.ack {
            self.rgate.ack_msg(self.source.msg);
        }
    }
}

/// Marshalls a message from `$args` and sends it via `$sg`, using `$rg` to receive the reply.
#[macro_export]
macro_rules! send_vmsg {
    ( $sg:expr, $rg:expr, $( $args:expr ),* ) => ({
        let mut os = $crate::com::GateOStream::default();
        $( os.push(&$args); )*
        os.send($sg, $rg)
    });
}

/// Marshalls a message from `$args` and sends it as a reply to the given `GateIStream`.
#[macro_export]
macro_rules! reply_vmsg {
    ( $is:expr, $( $args:expr ),* ) => ({
        let mut os = $crate::com::GateOStream::default();
        $( os.push(&$args); )*
        $is.reply_os(&os)
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
    rgate.receive(None)
}

/// Receives a message from `rgate` as a reply to the message that has been sent over `sgate` and
/// returns a [`GateIStream`] for the message.
#[inline(always)]
pub fn recv_reply<'r>(
    rgate: &'r RecvGate,
    sgate: Option<&SendGate>,
) -> Result<GateIStream<'r>, Error> {
    rgate.receive(sgate)
}

/// Receives a message from `$rg` and unmarshalls the message into the given arguments.
#[macro_export]
macro_rules! recv_vmsg {
    ( $rg:expr, $x:ty ) => ({
        match $crate::com::recv_msg($rg) {
            Err(e)      => Err(e),
            Ok(mut is)  => Ok(( is.pop::<$x>(), )),
        }
    });

    ( $rg:expr, $x1:ty, $($xs:ty),+ ) => ({
        match $crate::com::recv_msg($rg) {
            Err(e)      => Err(e),
            Ok(mut is)  => Ok(( is.pop::<$x1>(), $( is.pop::<$xs>() ),+ )),
        }
    });
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
    let res: u32 = reply.pop();
    match res {
        0 => Ok(reply),
        e => Err(Error::from(e)),
    }
}

/// Marshalls a message from `$args` and sends it via `$sg`, using `$rg` to receive the reply.
/// Afterwards, it waits for the reply and returns the `GateIStream` for the reply.
#[macro_export]
macro_rules! send_recv {
    ( $sg:expr, $rg:expr, $( $args:expr ),* ) => ({
        let mut os = $crate::com::GateOStream::default();
        $( os.push(&$args); )*
        os.call($sg, $rg)
    });
}

/// Marshalls a message from `$args` and sends it via `$sg`, using `$rg` to receive the reply.
/// Afterwards, it waits for the reply and unmarshalls the result (error code). If the result is an
/// error, it returns the error and otherwise the `GateIStream` for the reply.
#[macro_export]
macro_rules! send_recv_res {
    ( $sg:expr, $rg:expr, $( $args:expr ),* ) => ({
        send_recv!($sg, $rg, $( $args ),* ).and_then(|mut reply| {
            let res: u32 = reply.pop();
            match res {
                0 => Ok(reply),
                e => Err(Error::from(e)),
            }
        })
    });
}
