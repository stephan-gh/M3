/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

use crate::cap::{CapFlags, Selector};
use crate::com::{RGateArgs, RecvGate, SGateArgs, SendGate};
use crate::errors::Error;
use crate::int_enum;
use crate::kif::{CapRngDesc, CapType};
use crate::math;
use crate::mem::{self, MaybeUninit, MsgBuf};
use crate::net::{IpAddr, Port, Sd};
use crate::pes::VPE;
use crate::rc::Rc;
use crate::tcu::Message;

const MSG_SIZE: usize = 2048;
const MSG_CREDITS: usize = 4;
const MSG_BUF_SIZE: usize = MSG_SIZE * MSG_CREDITS;

const REPLY_SIZE: usize = 32;
const REPLY_BUF_SIZE: usize = REPLY_SIZE * MSG_CREDITS;

// the receive buffer slots are 2048 bytes, but we need to substract the TCU header and the other
// fields in DataMessage.
pub const MTU: usize = MSG_SIZE - (16 + 5 * mem::size_of::<u64>());

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
    pub sd: u64,
    pub addr: u64,
    pub port: u64,
    pub size: u64,
    pub data: [u8; MTU],
}

#[repr(C)]
pub struct ConnectedMessage {
    ty: u64,
    pub sd: u64,
    pub remote_addr: u64,
    pub remote_port: u64,
}

#[repr(C)]
pub struct ClosedMessage {
    ty: u64,
    pub sd: u64,
}

#[repr(C)]
pub struct CloseReqMessage {
    ty: u64,
    pub sd: u64,
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

    pub fn can_send(&self) -> Result<bool, Error> {
        self.sgate.can_send()
    }

    pub fn has_events(&self) -> bool {
        self.rgate.has_msgs()
    }

    pub fn receive_event(self: &Rc<Self>) -> Option<NetEvent> {
        self.rgate
            .fetch()
            .map(|msg| NetEvent::new(msg, self.clone()))
    }

    pub fn send_data<F>(
        &self,
        sd: Sd,
        addr: IpAddr,
        port: Port,
        size: usize,
        populate: F,
    ) -> Result<(), Error>
    where
        F: FnOnce(&mut [u8]),
    {
        let mut msg = DataMessage {
            ty: NetEventType::DATA.val,
            sd: sd as u64,
            addr: addr.0 as u64,
            port: port as u64,
            size: size as u64,
            // safety: data[0..size] will be initialized below; the rest will not be sent
            data: unsafe { MaybeUninit::uninit().assume_init() },
        };
        assert!(size <= msg.data.len());

        populate(&mut msg.data[0..size]);

        // in case the application is doing many sends in a row, make sure that we fetch and ack the
        // replies from the server. otherwise we stop getting the credits for our sgate back.
        self.fetch_replies();

        let msg_size = 5 * mem::size_of::<u64>() + size;
        self.sgate
            .send_aligned(&msg as *const _ as *const u8, msg_size, &self.rpl_gate)
    }

    pub fn send_connected(
        &self,
        sd: Sd,
        remote_addr: IpAddr,
        remote_port: Port,
    ) -> Result<(), Error> {
        let mut msg_buf = MsgBuf::borrow_def();
        msg_buf.set(ConnectedMessage {
            ty: NetEventType::CONNECTED.val,
            sd: sd as u64,
            remote_addr: remote_addr.0 as u64,
            remote_port: remote_port as u64,
        });
        self.sgate.send(&msg_buf, &self.rpl_gate)
    }

    pub fn send_closed(&self, sd: Sd) -> Result<(), Error> {
        let mut msg_buf = MsgBuf::borrow_def();
        msg_buf.set(ClosedMessage {
            ty: NetEventType::CLOSED.val,
            sd: sd as u64,
        });
        self.sgate.send(&msg_buf, &self.rpl_gate)
    }

    pub fn send_close_req(&self, sd: Sd) -> Result<(), Error> {
        let mut msg_buf = MsgBuf::borrow_def();
        msg_buf.set(CloseReqMessage {
            ty: NetEventType::CLOSE_REQ.val,
            sd: sd as u64,
        });
        self.sgate.send(&msg_buf, &self.rpl_gate)
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
            VPE::cur()
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

    pub fn sd(&self) -> Sd {
        match self.msg_type() {
            NetEventType::DATA => self.msg::<DataMessage>().sd as Sd,
            NetEventType::CONNECTED => self.msg::<ConnectedMessage>().sd as Sd,
            NetEventType::CLOSED => self.msg::<ClosedMessage>().sd as Sd,
            NetEventType::CLOSE_REQ => self.msg::<CloseReqMessage>().sd as Sd,
            _ => unreachable!(),
        }
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
