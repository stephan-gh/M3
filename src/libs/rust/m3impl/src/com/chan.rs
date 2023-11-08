/*
 * Copyright (C) 2022 Sebastian Ertel, Barkhausen Institut
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

//! A synchronous uni-directional communication channel, similar to `std::sync::mpsc::sync_channel`.
//!
//! A channel consists of a sender and a receiver and allows the sender to send messages to the
//! receiver. These messages are not buffered, but delivered synchronously to the receiver.
//! Therefore, this channel is comparable to a `std::sync::mpsc::sync_channel` with a bound of 0.
//!
//! This channel is simpler than a manual usage of [`SendGate`](crate::com::SendGate) and
//! [`RecvGate`](crate::com::RecvGate), but also more limited, because all communication is
//! synchronous, happens between exactly one sender and one receiver, and each transfer only
//! delivers a single data type.

use crate::cap::Selector;
use crate::com::stream::recv_msg;
use crate::com::{rgate::ReceivingGate, GateCap, RecvCap, RecvGate, SGateArgs, SendCap, SendGate};
use crate::errors::{Code, Error};
use crate::serialize::{Deserialize, Serialize};

use crate::util::math;

/// Represents the capability for sender part of the channel, which needs to be turned into a
/// `Sender` before it can be used.
pub struct SenderCap {
    scap: SendCap,
}

impl SenderCap {
    fn new<R: ReceivingGate>(rgate: &R) -> Result<Self, Error> {
        let scap = SendCap::new_with(SGateArgs::new(rgate).credits(1))?;
        Ok(Self { scap })
    }

    /// Returns the selector of the underlying [`SendCap`](crate::com::SendCap).
    ///
    /// This method is used to delegate the sending part of the channel to another activity.
    pub fn sel(&self) -> Selector {
        self.scap.sel()
    }

    /// Activates the underyling [`SendCap`](crate::com::SendCap) and thereby turns this `SenderCap`
    /// into a `Sender`.
    pub fn activate(self) -> Result<Sender, Error> {
        Ok(Sender {
            sgate: self.scap.activate()?,
        })
    }
}

/// Represents the sender part of the channel created with [`sync_channel`].
pub struct Sender {
    sgate: SendGate,
}

impl Sender {
    /// Creates a new [`Sender`] that is bound to given selector.
    ///
    /// This function is intended to be used by the communication partner that did not create the
    /// channel, but wants to connect to the sending part of it.
    pub fn new_bind(sel: Selector) -> Result<Self, Error> {
        let sgate = SendGate::new_bind(sel)?;
        Ok(Self { sgate })
    }

    /// Sends the given item synchronously to the receiver
    pub fn send<T: Serialize>(&self, data: T) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), data).map(|_| ())
    }
}

/// Represents the capability for receiver part of the channel, which needs to be turned into a
/// `Receiver` before it can be used.
pub struct ReceiverCap {
    rcap: RecvCap,
}

impl ReceiverCap {
    fn new(msg_size: usize) -> Result<Self, Error> {
        let rcap = RecvCap::new(math::next_log2(msg_size), math::next_log2(msg_size))?;
        Ok(Self { rcap })
    }

    /// Returns the selector of the underlying [`RecvCap`](crate::com::RecvCap).
    ///
    /// This method is used to delegate the receiver part of the channel to another activity.
    pub fn sel(&self) -> Selector {
        self.rcap.sel()
    }

    /// Activates the underyling [`RecvCap`](crate::com::RecvCap) and thereby turns this
    /// `ReceiverCap` into a `Receiver`.
    pub fn activate(self) -> Result<Receiver, Error> {
        Ok(Receiver {
            rgate: self.rcap.activate()?,
        })
    }
}

/// Represents the receiver part of the channel created with [`sync_channel`].
pub struct Receiver {
    rgate: RecvGate,
}

impl Receiver {
    /// Creates a new [`Receiver`] that is bound to given selector.
    ///
    /// This function is intended to be used by the communication partner that did not create the
    /// channel, but wants to connect to the receiver part of it.
    pub fn new_bind(sel: Selector) -> Result<Self, Error> {
        let rgate = RecvGate::new_bind(sel)?;
        Ok(Self { rgate })
    }

    /// Receives an item of given type from the sender
    pub fn recv<T: Deserialize<'static>>(&self) -> Result<T, Error> {
        let mut s = recv_msg(&self.rgate)?;
        // return credits for sending
        reply_vmsg!(s, Code::Success)?;
        s.pop::<T>()
    }
}

/// Creates a new synchronous communication channel with default settings (256b message size)
pub fn sync_channel() -> Result<(SenderCap, ReceiverCap), Error> {
    sync_channel_with(256)
}

/// Creates a new synchronous communication channel with the given maximum message size
pub fn sync_channel_with(msg_size: usize) -> Result<(SenderCap, ReceiverCap), Error> {
    let rx = ReceiverCap::new(msg_size)?;
    let tx = SenderCap::new(&rx.rcap)?;
    Ok((tx, rx))
}

/// Convenience macro that constructs a closure containing code block `$b` and runs it with `$act`
/// with given communication channels.
///
/// The closure is constructed with the arguments `$b_chans` and their types `$b_types` and is
/// called with the channels `$chans`.
///
/// See `rustunittests::tactivity::run_send_receive_chan_macro` for an example usage.
#[macro_export]
macro_rules! run_with_channels {
    ($act:expr, | $($b_chans:ident : $b_types:ty),+ $(,)? | $b:block ( $($chans:ident),+ ) ) => {
        (|| {
            let mut act = $act;
            $( act.delegate_obj($chans.sel())?; )+
            let mut sink = act.data_sink();
            $( sink.push($chans.sel()); )+

            act.run(|| {
                let mut source = Activity::own().data_source();
                $( let $b_chans = <$b_types>::new_bind(source.pop()?)?; )+
                $b
            })
        })()
    }
}
