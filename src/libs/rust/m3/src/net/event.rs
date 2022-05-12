/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

use core::fmt;

use crate::cap::{CapFlags, Selector};
use crate::com::{RGateArgs, RecvGate, SGateArgs, SendGate};
use crate::errors::Error;
use crate::int_enum;
use crate::kif::{CapRngDesc, CapType};
use crate::math;
use crate::mem::{self, MaybeUninit, MsgBuf};
use crate::net::{Endpoint, IpAddr, Port};
use crate::rc::Rc;
use crate::tcu::{Header, Message};
use crate::tiles::Activity;

const MSG_SIZE: usize = 2048;
const MSG_CREDITS: usize = 4;
const MSG_BUF_SIZE: usize = MSG_SIZE * MSG_CREDITS;

const REPLY_SIZE: usize = 32;
const REPLY_BUF_SIZE: usize = REPLY_SIZE * MSG_CREDITS;

// the receive buffer slots are 2048 bytes, but we need to substract the TCU header and the other
// fields in DataMessage.
pub const MTU: usize = MSG_SIZE - (mem::size_of::<Header>() + 4 * mem::size_of::<u64>());

int_enum! {
    pub struct NetEventType : u64 {
        const DATA          = 0;
        const CONNECTED     = 1;
        const CLOSED        = 2;
        const CLOSE_REQ     = 3;
    }
}

#[repr(C, align(2048))]
pub struct DataMessage {
    ty: u64,
    pub addr: u64,
    pub port: u64,
    pub size: u64,
    pub data: [u8; MTU],
}

#[repr(C)]
pub struct ConnectedMessage {
    ty: u64,
    pub remote_addr: u64,
    pub remote_port: u64,
}

impl ConnectedMessage {
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            ty: NetEventType::CONNECTED.val,
            remote_addr: endpoint.addr.0 as u64,
            remote_port: endpoint.port as u64,
        }
    }
}

impl fmt::Debug for ConnectedMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "remote={}",
            Endpoint::new(IpAddr(self.remote_addr as u32), self.remote_port as Port)
        )
    }
}

#[repr(C)]
pub struct ClosedMessage {
    ty: u64,
}

impl Default for ClosedMessage {
    fn default() -> Self {
        Self {
            ty: NetEventType::CLOSED.val,
        }
    }
}

impl fmt::Debug for ClosedMessage {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

#[repr(C)]
pub struct CloseReqMessage {
    ty: u64,
}

impl Default for CloseReqMessage {
    fn default() -> Self {
        Self {
            ty: NetEventType::CLOSE_REQ.val,
        }
    }
}

impl fmt::Debug for CloseReqMessage {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

#[derive(Eq, PartialEq)]
enum NetEventSide {
    Client,
    Server,
}

pub struct NetEventChannel {
    side: NetEventSide,
    rgate: RecvGate,
    rpl_gate: RecvGate,
    sgate: SendGate,
}

impl NetEventChannel {
    pub fn new_server(caps: Selector) -> Result<Rc<Self>, Error> {
        let mut rgate = RecvGate::new_with(
            RGateArgs::default()
                .sel(caps + 0)
                .msg_order(math::next_log2(MSG_SIZE))
                .order(math::next_log2(MSG_BUF_SIZE)),
        )?;
        rgate.activate()?;

        SendGate::new_with(
            SGateArgs::new(&rgate)
                .sel(caps + 3)
                .credits(MSG_CREDITS as u32)
                .flags(CapFlags::KEEP_CAP),
        )?;

        let rgate_cli = RecvGate::new_with(
            RGateArgs::default()
                .sel(caps + 2)
                .msg_order(math::next_log2(MSG_SIZE))
                .order(math::next_log2(MSG_BUF_SIZE))
                .flags(CapFlags::KEEP_CAP),
        )?;
        let sgate = SendGate::new_with(
            SGateArgs::new(&rgate_cli)
                .sel(caps + 1)
                .credits(MSG_CREDITS as u32),
        )?;

        let mut rpl_gate =
            RecvGate::new(math::next_log2(REPLY_BUF_SIZE), math::next_log2(REPLY_SIZE))?;
        rpl_gate.activate()?;

        Ok(Rc::new(Self {
            side: NetEventSide::Server,
            rgate,
            rpl_gate,
            sgate,
        }))
    }

    pub fn new_client(caps: Selector) -> Result<Rc<Self>, Error> {
        let mut rgate = RecvGate::new_bind(
            caps + 0,
            math::next_log2(MSG_BUF_SIZE),
            math::next_log2(MSG_SIZE),
        );
        rgate.activate()?;

        let mut rpl_gate =
            RecvGate::new(math::next_log2(REPLY_BUF_SIZE), math::next_log2(REPLY_SIZE))?;
        rpl_gate.activate()?;

        Ok(Rc::new(Self {
            side: NetEventSide::Client,
            rgate,
            rpl_gate,
            sgate: SendGate::new_bind(caps + 1),
        }))
    }

    pub fn wait_for_events(&self) {
        // ignore errors
        Activity::own()
            .wait_for(Some(self.rgate.ep().unwrap()), None, None)
            .ok();
    }

    pub fn wait_for_credits(&self) {
        // ignore errors
        Activity::own()
            .wait_for(Some(self.rpl_gate.ep().unwrap()), None, None)
            .ok();
    }

    pub fn can_send(&self) -> Result<bool, Error> {
        self.sgate.can_send()
    }

    pub fn has_events(&self) -> bool {
        self.rgate.has_msgs()
    }

    pub fn has_all_credits(&self) -> bool {
        self.sgate.credits().unwrap() == MSG_CREDITS as u32
    }

    pub fn receive_event(self: &Rc<Self>) -> Option<NetEvent> {
        self.rgate
            .fetch()
            .map(|msg| NetEvent::new(msg, self.clone()))
    }

    pub fn send_event<E>(&self, event: E) -> Result<(), Error> {
        let mut msg_buf = MsgBuf::borrow_def();
        msg_buf.set(event);
        self.sgate.send(&msg_buf, &self.rpl_gate)
    }

    pub fn build_data_message<F>(&self, endpoint: Endpoint, size: usize, populate: F) -> DataMessage
    where
        F: FnOnce(&mut [u8]),
    {
        assert!(size <= MTU);

        #[allow(clippy::uninit_assumed_init)]
        let mut msg = DataMessage {
            ty: NetEventType::DATA.val,
            addr: endpoint.addr.0 as u64,
            port: endpoint.port as u64,
            size: size as u64,
            // safety: data[0..size] will be initialized below; the rest will not be sent
            data: unsafe { MaybeUninit::uninit().assume_init() },
        };

        populate(&mut msg.data[0..size]);
        msg
    }

    pub fn send_data(&self, msg: &DataMessage) -> Result<(), Error> {
        // in case the application is doing many sends in a row, make sure that we fetch and ack the
        // replies from the server. otherwise we stop getting the credits for our sgate back.
        self.fetch_replies();

        let msg_size = 4 * mem::size_of::<u64>() + msg.size as usize;
        self.sgate
            .send_aligned(msg as *const _ as *const u8, msg_size, &self.rpl_gate)
    }

    pub fn fetch_replies(&self) {
        while let Some(reply) = self.rpl_gate.fetch() {
            self.rpl_gate.ack_msg(reply).unwrap();
        }
    }
}

impl Drop for NetEventChannel {
    fn drop(&mut self) {
        if self.side == NetEventSide::Server {
            // revoke client caps
            Activity::own()
                .revoke(
                    CapRngDesc::new(CapType::OBJECT, self.rgate.sel() + 2, 2),
                    false,
                )
                .unwrap();
        }
    }
}

pub struct NetEvent {
    msg: &'static Message,
    channel: Rc<NetEventChannel>,
    ack: bool,
}

impl NetEvent {
    fn new(msg: &'static Message, channel: Rc<NetEventChannel>) -> Self {
        Self {
            msg,
            channel,
            ack: true,
        }
    }

    pub fn msg_type(&self) -> NetEventType {
        NetEventType::from(*self.msg.get_data::<u64>())
    }

    pub fn msg<T>(&self) -> &T {
        // TODO improve that
        unsafe { self.msg.get_data_unchecked::<T>() }
    }
}

impl Drop for NetEvent {
    fn drop(&mut self) {
        if self.ack {
            // reply empty message; ignore failures here
            let reply = MsgBuf::borrow_def();
            self.channel.rgate.reply(&reply, self.msg).ok();
        }
    }
}
