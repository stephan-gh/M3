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

use crate::errors::{Code, Error};
use crate::net::{socket::Socket, IpAddr, NetData, SocketState, SocketType};
use crate::session::NetworkManager;

/// TCP socket state according to the tcp state machine.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TcpState {
    Closed      = 0,
    Listen      = 1,
    SynSent     = 2,
    SynReceived = 3,
    Established = 4,
    FinWait1    = 5,
    FinWait2    = 6,
    CloseWait   = 7,
    Closing     = 8,
    LastAck     = 9,
    TimeWait    = 10,
    Invalid     = 11,
}

impl TcpState {
    pub fn from_u64(other: u64) -> TcpState {
        match other {
            0 => TcpState::Closed,
            1 => TcpState::Listen,
            2 => TcpState::SynSent,
            3 => TcpState::SynReceived,
            4 => TcpState::Established,
            5 => TcpState::FinWait1,
            6 => TcpState::FinWait2,
            7 => TcpState::CloseWait,
            8 => TcpState::Closing,
            9 => TcpState::LastAck,
            10 => TcpState::TimeWait,
            _ => TcpState::Invalid,
        }
    }
}

pub struct TcpSocket<'a> {
    /// If set to true, the socket will always wait for operations to finish before returning.
    // TODO maybe there should be a timeout version for blocking as well, similar to what the std lib is doing for the mpsc channels.
    blocking: bool,
    socket: Socket<'a>,
}

impl<'a> TcpSocket<'a> {
    pub fn new(network_manager: &'a NetworkManager) -> Result<Self, Error> {
        Ok(TcpSocket {
            blocking: false,
            socket: Socket::new(SocketType::Stream, network_manager, None)?,
        })
    }

    /// Waits for the socket to change into some state
    fn wait_for_state(&self, target_state: TcpState) -> Result<(), Error> {
        while self.state()? != target_state {
            // TODO should notify the scheduler to let this thread sleep probably.
        }
        Ok(())
    }

    /// Sets the blocking state.
    /// If set to `true`, the socket will always wait for operations to be finished.
    /// For instance when calling `connect()`, the socket returns once a connection is established.
    pub fn set_blocking(&mut self, should_block: bool) {
        self.blocking = should_block;
    }

    pub fn connect(
        &mut self,
        remote_addr: IpAddr,
        remote_port: u16,
        local_addr: IpAddr,
        local_port: u16,
    ) -> Result<(), Error> {
        self.socket.nm.connect(
            self.socket.sd,
            remote_addr,
            remote_port,
            local_addr,
            local_port,
        )?;
        if self.blocking {
            self.wait_for_state(TcpState::Established)?;
        }

        Ok(())
    }

    pub fn listen(&mut self, local_addr: IpAddr, local_port: u16) -> Result<(), Error> {
        self.socket
            .nm
            .listen(self.socket.sd, local_addr, local_port)?;
        if self.blocking {
            self.wait_for_state(TcpState::Listen)?;
        }
        Ok(())
    }

    pub fn recv(&mut self) -> Result<NetData, Error> {
        if self.blocking {
            loop {
                match self.socket.nm.recv(self.socket.sd) {
                    Ok(rcv) => return Ok(rcv),
                    Err(e) => match e.code() {
                        Code::NoSuchSocket | Code::SocketClosed | Code::InvState => {
                            return Err(Error::from(e));
                        },
                        _ => {}, // ignore and keep waiting
                    },
                }
            }
        }
        else {
            self.socket.nm.recv(self.socket.sd)
        }
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Do not specify addresses, is handled by the server through listen/connect
        // Note that there is currently no way to check if something was actually send, so there is no real blocking as well.
        self.socket.nm.send(
            self.socket.sd,
            IpAddr::unspecified(),
            0,
            IpAddr::unspecified(),
            0,
            data,
        )
    }

    /// Queries the socket state from the server. Can be used to wait for the socket to change into a specific state.
    pub fn state(&self) -> Result<TcpState, Error> {
        let state = self.socket.nm.get_state(self.socket.sd)?;
        if let SocketState::TcpState(st) = state {
            Ok(st)
        }
        else {
            Err(Error::new(Code::WrongSocketType))
        }
    }

    /// Sends a close request. Can be used to gracefully shut down. When the Socket is dropped close is send anyways, but can be waited
    /// upon close acknowledgment by the connection.
    pub fn close(&self) -> Result<(), Error> {
        self.socket.nm.close(self.socket.sd)?;
        if self.blocking {
            if let Err(e) = self.wait_for_state(TcpState::Closed) {
                // If we got a NotSup, then the socket was already deleted on the server when we last checked the state,
                // therefore we can just return
                match e.code() {
                    Code::NotSup => return Ok(()),
                    _ => return Err(e),
                }
            }
        }
        Ok(())
    }
}
