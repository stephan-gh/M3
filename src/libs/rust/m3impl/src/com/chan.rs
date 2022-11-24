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
use crate::com::{RecvGate, SGateArgs, SendGate};
use crate::errors::{Code, Error};
use crate::serialize::{Deserialize, Serialize};
use crate::tcu;
use crate::util::math;

/// Represents the sender part of the channel created with [`sync_channel`].
pub struct Sender {
    sgate: SendGate,
}

impl Sender {
    fn new(rgate: &RecvGate) -> Result<Self, Error> {
        let sgate = SendGate::new_with(SGateArgs::new(rgate).credits(1))?;
        Ok(Self { sgate })
    }

    /// Creates a new [`Sender`] that is bound to given selector.
    ///
    /// This function is intended to be used by the communication partner that did not create the
    /// channel, but wants to connect to the sending part of it.
    pub fn new_bind(sel: Selector) -> Self {
        let sgate = SendGate::new_bind(sel);
        Self { sgate }
    }

    /// Returns the selector of the underlying [`SendGate`](crate::com::SendGate).
    ///
    /// This method is used to delegate the sending part of the channel to another activity.
    pub fn sel(&self) -> Selector {
        self.sgate.sel()
    }

    /// Sends the given item synchronously to the receiver
    pub fn send<T: Serialize>(&self, data: T) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), data).map(|_| ())
    }

    /// Manually activates the underyling [`SendGate`](crate::com::SendGate).
    ///
    /// The [`SendGate`](crate::com::SendGate) is activated automatically on first use. In case
    /// automatic activation is not possible (e.g., would cause a deadlock), manual activation can
    /// be used.
    pub fn activate(&self) -> Result<tcu::EpId, Error> {
        self.sgate.activate()
    }
}

/// Represents the receiver part of the channel created with [`sync_channel`].
pub struct Receiver {
    rgate: RecvGate,
}

impl Receiver {
    fn new(msg_size: usize) -> Result<Self, Error> {
        let rgate = RecvGate::new(math::next_log2(msg_size), math::next_log2(msg_size))?;
        Ok(Self { rgate })
    }

    /// Creates a new [`Receiver`] that is bound to given selector.
    ///
    /// This function is intended to be used by the communication partner that did not create the
    /// channel, but wants to connect to the receiver part of it.
    pub fn new_bind(sel: Selector) -> Self {
        let rgate = RecvGate::new_bind(sel);
        Self { rgate }
    }

    /// Returns the selector of the underlying [`RecvGate`](crate::com::RecvGate).
    ///
    /// This method is used to delegate the receiver part of the channel to another activity.
    pub fn sel(&self) -> Selector {
        self.rgate.sel()
    }

    /// Receives an item of given type from the sender
    pub fn recv<T: Deserialize<'static>>(&self) -> Result<T, Error> {
        let mut s = recv_msg(&self.rgate)?;
        // return credits for sending
        reply_vmsg!(s, Code::Success)?;
        s.pop::<T>()
    }

    /// Manually activates the underyling [`RecvGate`](crate::com::RecvGate).
    ///
    /// The [`RecvGate`](crate::com::RecvGate) is activated automatically on first use. In case
    /// automatic activation is not possible (e.g., would cause a deadlock), manual activation can
    /// be used.
    pub fn activate(&self) -> Result<tcu::EpId, Error> {
        self.rgate.activate()
    }
}

/// Creates a new synchronous communication channel with default settings (256b message size)
pub fn sync_channel() -> Result<(Sender, Receiver), Error> {
    sync_channel_with(256)
}

/// Creates a new synchronous communication channel with the given maximum message size
pub fn sync_channel_with(msg_size: usize) -> Result<(Sender, Receiver), Error> {
    let rx = Receiver::new(msg_size)?;
    let tx = Sender::new(&rx.rgate)?;
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
    ($act:expr, | $($b_chans:ident : $b_types:ty),+ | $b:block ( $($chans:ident),+ ) ) => {
        (|| {
            let mut act = $act;
            $( act.delegate_obj($chans.sel())?; )+
            let mut sink = act.data_sink();
            $( sink.push($chans.sel()); )+

            act.run(|| {
                let mut source = Activity::own().data_source();
                $( let $b_chans = <$b_types>::new_bind(source.pop()?); )+
                $b
            })
        })()
    }
}
