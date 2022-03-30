/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

use core::any::Any;
use core::fmt;

use crate::boxed::Box;
use crate::errors::{Code, Error};
use crate::io;
use crate::net::{
    event,
    socket::{Socket, SocketArgs, State},
    Endpoint, Port, SocketType,
};
use crate::rc::Rc;
use crate::session::{HashInput, HashOutput, NetworkManager};
use crate::tiles::Activity;
use crate::vfs::{self, Fd, File, FileEvent, FileRef, INV_FD};

/// Configures the buffer sizes for stream sockets
pub struct StreamSocketArgs {
    nm: Rc<NetworkManager>,
    args: SocketArgs,
}

impl StreamSocketArgs {
    /// Creates a new [`StreamSocketArgs`] with default settings
    pub fn new(nm: Rc<NetworkManager>) -> Self {
        Self {
            nm,
            args: SocketArgs::default(),
        }
    }

    /// Sets the size in bytes of the receive buffer
    pub fn recv_buffer(mut self, size: usize) -> Self {
        self.args.rbuf_size = size;
        self
    }

    /// Sets the size in bytes of the send buffer
    pub fn send_buffer(mut self, size: usize) -> Self {
        self.args.sbuf_size = size;
        self
    }
}

/// Represents a stream socket using the transmission control protocol (TCP)
pub struct TcpSocket {
    fd: Fd,
    socket: Socket,
    nm: Rc<NetworkManager>,
}

impl TcpSocket {
    /// Creates a new TCP sockets with given arguments.
    ///
    /// By default, the socket is in blocking mode, that is, all functions
    /// ([`connect`](TcpSocket::connect), [`send`](TcpSocket::send), [`recv`](TcpSocket::recv),
    /// ...) do not return until the operation is complete. This can be changed via
    /// [`set_blocking`](TcpSocket::set_blocking).
    pub fn new(args: StreamSocketArgs) -> Result<FileRef<Self>, Error> {
        let sock = Box::new(TcpSocket {
            socket: args.nm.create(SocketType::Stream, None, &args.args)?,
            nm: args.nm,
            fd: INV_FD,
        });
        let fd = Activity::cur().files().add(sock)?;
        Ok(FileRef::new_owned(fd))
    }

    /// Returns the current state of the socket
    pub fn state(&self) -> State {
        self.socket.state()
    }

    /// Returns the local endpoint
    ///
    /// The local endpoint is only `Some` if the socket has been put into listen mode via
    /// [`listen`](TcpSocket::listen) or was connected to a remote endpoint via
    /// [`connect`](TcpSocket::connect).
    pub fn local_endpoint(&self) -> Option<Endpoint> {
        self.socket.local_ep
    }

    /// Returns the remote endpoint
    ///
    /// The remote endpoint is only `Some`, if the socket is currently connected (achieved either
    /// via [`connect`](TcpSocket::connect) or [`accept`](TcpSocket::accept)). Otherwise, the remote
    /// endpoint is `None`.
    pub fn remote_endpoint(&self) -> Option<Endpoint> {
        self.socket.remote_ep
    }

    /// Puts this socket into listen mode on the given port.
    ///
    /// In listen mode, remote connections can be accepted. See [`accept`](TcpSocket::accept). Note
    /// that in contrast to conventional TCP/IP stacks, [`listen`](TcpSocket::listen) is a
    /// combination of the traditional `bind` and `listen`.
    ///
    /// Listing on this port requires that the used session has permission for this port. This is
    /// controlled with the "tcp=..." argument in the session argument of M³'s config files.
    ///
    /// Returns an error if the socket is not in state [`Closed`](State::Closed).
    pub fn listen(&mut self, port: Port) -> Result<(), Error> {
        if self.socket.state() != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        let addr = self.nm.listen(self.socket.sd(), port)?;
        self.socket.local_ep = Some(Endpoint::new(addr, port));
        self.socket.state = State::Listening;
        Ok(())
    }

    /// Connects this socket to the given remote endpoint.
    pub fn connect(&mut self, endpoint: Endpoint) -> Result<(), Error> {
        if self.state() == State::Connected {
            if self.remote_endpoint().unwrap() != endpoint {
                return Err(Error::new(Code::IsConnected));
            }
            return Ok(());
        }
        if self.state() == State::RemoteClosed {
            return Err(Error::new(Code::InvState));
        }

        if self.state() == State::Connecting {
            return Err(Error::new(Code::AlreadyInProgress));
        }

        let local_ep = self.nm.connect(self.socket.sd(), endpoint)?;
        self.socket.state = State::Connecting;
        self.socket.remote_ep = Some(endpoint);
        self.socket.local_ep = Some(local_ep);

        if !self.is_blocking() {
            return Err(Error::new(Code::InProgress));
        }

        while self.state() == State::Connecting {
            self.socket.wait_for_events();
        }

        if self.state() != State::Connected {
            Err(Error::new(Code::ConnectionFailed))
        }
        else {
            Ok(())
        }
    }

    /// Accepts a remote connection on this socket
    ///
    /// The socket has to be put into listen mode first. Note that in contrast to conventional
    /// TCP/IP stacks, accept does not yield a new socket, but uses this socket for the accepted
    /// connection. Thus, to support multiple connections to the same port, put multiple sockets in
    /// listen mode on this port and call accept on each of them.
    pub fn accept(&mut self) -> Result<Endpoint, Error> {
        if self.state() == State::Connected {
            return Ok(self.remote_endpoint().unwrap());
        }
        if self.state() == State::Connecting {
            return Err(Error::new(Code::AlreadyInProgress));
        }
        if self.state() != State::Listening {
            return Err(Error::new(Code::InvState));
        }

        self.socket.state = State::Connecting;
        while self.state() == State::Connecting {
            if !self.is_blocking() {
                return Err(Error::new(Code::InProgress));
            }
            self.socket.wait_for_events();
        }

        if self.state() != State::Connected {
            Err(Error::new(Code::ConnectionFailed))
        }
        else {
            Ok(self.remote_endpoint().unwrap())
        }
    }

    /// Returns whether data can currently be received from the socket
    ///
    /// Note that this function does not process events. To receive data, any receive function on
    /// this socket or [`NetworkManager::wait`] has to be called.
    pub fn has_data(&self) -> bool {
        self.socket.has_data()
    }

    /// Receives data from the socket into the given buffer.
    ///
    /// The socket has to be connected first (either via [`connect`](TcpSocket::connect) or
    /// [`accept`](TcpSocket::accept)). Note that data can be received after the remote side has
    /// closed the socket (state [`RemoteClosed`](State::RemoteClosed)), but not if this side has
    /// been closed.
    ///
    /// Returns the number of received bytes.
    pub fn recv(&mut self, data: &mut [u8]) -> Result<usize, Error> {
        match self.socket.state() {
            // receive is possible with an established connection or a connection that that has
            // already been closed by the remote side
            State::Connected | State::RemoteClosed => {
                self.socket.next_data(data.len(), |buf, _ep| {
                    data[0..buf.len()].copy_from_slice(buf);
                    (buf.len(), buf.len())
                })
            },
            _ => Err(Error::new(Code::NotConnected)),
        }
    }

    /// Sends the given data to this socket
    ///
    /// The socket has to be connected first (either via [`connect`](TcpSocket::connect) or
    /// [`accept`](TcpSocket::accept)). Note that data can be received after the remote side has
    /// closed the socket (state [`RemoteClosed`](State::RemoteClosed)), but not if this side has
    /// been closed.
    ///
    /// Returns the number of sent bytes or an error. If an error occurs (e.g., remote side closed
    /// the socket) and some of the data has already been sent, the number of sent bytes is
    /// returned. Otherwise, the error is returned.
    pub fn send(&mut self, mut data: &[u8]) -> Result<usize, Error> {
        let mut total = 0;
        while data.len() > 0 {
            let amount = event::MTU.min(data.len());
            let res = match self.socket.state() {
                // like for receive: still allow sending if the remote side closed the connection
                State::Connected | State::RemoteClosed => self
                    .socket
                    .send(&data[0..amount], self.remote_endpoint().unwrap()),
                _ => Err(Error::new(Code::NotConnected)),
            };
            if let Err(e) = res {
                return match total {
                    0 => Err(e),
                    t => Ok(t),
                };
            }

            data = &data[amount..];
            total += amount;
        }
        Ok(total)
    }

    /// Closes the connection
    ///
    /// In contrast to [`abort`](TcpSocket::abort), close properly closes the connection to the
    /// remote endpoint by going through the TCP protocol.
    ///
    /// Note that [`close`](TcpSocket::close) is also called on drop.
    pub fn close(&mut self) -> Result<(), Error> {
        if self.state() == State::Closed {
            return Ok(());
        }

        if self.state() == State::Closing {
            return Err(Error::new(Code::AlreadyInProgress));
        }

        // send the close request
        loop {
            match self
                .socket
                .channel
                .send_event(event::CloseReqMessage::default())
            {
                Err(e) if e.code() == Code::NoCredits => {},
                Err(e) => return Err(e),
                Ok(_) => break,
            }

            if !self.is_blocking() {
                return Err(Error::new(Code::WouldBlock));
            }

            self.socket.wait_for_credits();
        }

        // ensure that we don't receive more data (which could block our event channel and thus
        // prevent us from receiving the closed event)
        self.socket.state = State::Closing;
        self.socket.recv_queue.clear();

        // now wait for the response; can be non-blocking
        while self.state() != State::Closed {
            if !self.is_blocking() {
                return Err(Error::new(Code::InProgress));
            }

            self.socket.wait_for_events();
        }
        Ok(())
    }

    /// Aborts the connection
    ///
    /// In contrast to [`close`](TcpSocket::close), this is a hard abort, which does not go through
    /// the TCP protocol, but simply "forgets" this socket. Furthermore, it is *not* guaranteed that
    /// all data has already been transmitted. Use [`close`](TcpSocket::close) if that is important.
    pub fn abort(&mut self) -> Result<(), Error> {
        self.nm.abort(self.socket.sd(), false)?;
        self.socket.recv_queue.clear();
        self.socket.disconnect();
        Ok(())
    }
}

impl File for TcpSocket {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn fd(&self) -> Fd {
        self.fd
    }

    fn set_fd(&mut self, fd: Fd) {
        self.fd = fd;
    }

    fn file_type(&self) -> u8 {
        // not supported
        b'\0'
    }

    fn is_blocking(&self) -> bool {
        self.socket.blocking()
    }

    /// Sets whether the socket is using blocking mode.
    ///
    /// In blocking mode, all functions ([`connect`](TcpSocket::connect), [`send`](TcpSocket::send),
    /// [`recv`](TcpSocket::recv), ...) do not return until the operation is complete. In
    /// non-blocking mode, all functions return in case they would need to block, that is, wait
    /// until an event is received or further data can be sent.
    fn set_blocking(&mut self, blocking: bool) -> Result<(), Error> {
        self.socket.set_blocking(blocking);
        Ok(())
    }

    fn check_events(&mut self, events: FileEvent) -> bool {
        self.socket.has_events(events)
    }
}

impl io::Read for TcpSocket {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.recv(buf)
    }
}

impl io::Write for TcpSocket {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.send(buf)
    }
}

impl vfs::Seek for TcpSocket {
}

impl vfs::Map for TcpSocket {
}

impl HashInput for TcpSocket {
}

impl HashOutput for TcpSocket {
}

impl fmt::Debug for TcpSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TcpSocket")
    }
}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        // use blocking mode here, because we cannot leave here until the socket is closed.
        self.set_blocking(true).unwrap();
        // ignore errors
        self.close().ok();

        self.nm.abort(self.socket.sd(), true).ok();
    }
}
