/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

use crate::cap::Selector;
use crate::com::{MemGate, RecvGate, SendGate};
use crate::errors::{Code, Error};
use crate::net::NetData;

use super::{MSG_BUF_ORDER, MSG_ORDER};

pub struct NetChannel {
    sg: SendGate,
    rg: RecvGate,

    #[allow(dead_code)]
    mem: MemGate, // TODO Used when socket as file is used?
}

impl NetChannel {
    /// Creates a new channel that is bound to `caps` and `caps+2`. Assumes that the `caps` where obtained from the netrs service, and are valid gates
    pub fn new_with_gates(send: SendGate, mut recv: RecvGate, mem: MemGate) -> Self {
        // activate rgate
        recv.activate().expect("Failed to activate server rgate");

        NetChannel {
            sg: send,
            rg: recv,
            mem,
        }
    }

    /// Does not crate new gates for this channel, but binds to them at `caps`-`caps+2`
    pub fn bind(caps: Selector) -> Result<Self, Error> {
        let mut rgate = RecvGate::new_bind(caps + 0, MSG_BUF_ORDER, MSG_ORDER);
        rgate.activate().expect("Failed to activate rgate");
        let sgate = SendGate::new_bind(caps + 1);
        let mgate = MemGate::new_bind(caps + 2);

        Ok(NetChannel {
            sg: sgate,
            rg: rgate,
            mem: mgate,
        })
    }

    /// Sends data over the send gate
    pub fn send(&self, net_data: NetData) -> Result<(), Error> {
        self.sg.send_aligned(
            &net_data as *const _ as *const u8,
            net_data.send_size(),
            &self.rg,
        )?;
        Ok(())
    }

    /// Tries to receive a message from the other side
    pub fn receive(&self) -> Result<NetData, Error> {
        // Fetch message by hand, if something is fetched,
        // assumes that it is a NetData package.
        if let Some(msg) = self.rg.fetch() {
            // TODO can we get around the clone?
            // safety: we know that we always receive NetData here and when receiving it, it doesn't
            // have to be 2048-byte aligned.
            let net_data = unsafe { msg.get_data_unchecked::<NetData>().clone() };
            // mark message as read
            self.rg.ack_msg(msg)?;

            Ok(net_data)
        }
        else {
            Err(Error::new(Code::NotSup))
        }
    }
}
