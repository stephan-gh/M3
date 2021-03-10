use m3::cap::Selector;
use m3::com::GateIStream;
use m3::errors::{Code, Error};
use m3::server::CapExchange;

pub mod file_session;
pub mod sockets;
pub use file_session::FileSession;
pub mod socket_session;
pub use socket_session::SocketSession;

use smoltcp::socket::SocketSet;

pub const MSG_SIZE: usize = 128;

pub enum NetworkSession {
    FileSession(FileSession),
    SocketSession(SocketSession),
}

impl NetworkSession {
    pub fn obtain(
        &mut self,
        crt: usize,
        server: Selector,
        xchg: &mut CapExchange,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.obtain(crt, server, xchg),
        }
    }

    pub fn delegate(&mut self, xchg: &mut CapExchange) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(fs) => fs.delegate(xchg),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn stat(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn seek(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn next_in(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn next_out(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn commit(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn close(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.close(is, socket_set),
        }
    }

    pub fn create(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.create(is, socket_set),
        }
    }

    pub fn bind(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.bind(is, socket_set),
        }
    }

    pub fn listen(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.listen(is, socket_set),
        }
    }

    pub fn connect(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.connect(is, socket_set),
        }
    }

    pub fn accept(
        &mut self,
        _is: &mut GateIStream,
        _socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn count(
        &mut self,
        _is: &mut GateIStream,
        _socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn query_state(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.query_state(is, socket_set),
        }
    }
}
