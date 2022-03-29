/*
 * Copyright (C) 2021, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
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

use crate::com::{RecvGate, SendGate, EP};
use crate::crypto::HashAlgorithm;
use crate::errors::{Code, Error};
use crate::int_enum;
use crate::session::ClientSession;

/// Represents a session at the hash multiplexer.
/// The state of previously hashed data will be maintained
/// until the session is destroyed.
pub struct HashSession {
    algo: &'static HashAlgorithm,
    _sess: ClientSession,
    sgate: SendGate,
    ep: EP,
}

int_enum! {
    /// The operations for the hash protocol.
    pub struct HashOp : u64 {
        const RESET = 0;
        const INPUT = 1;
        const OUTPUT = 2;
    }
}

impl HashSession {
    /// Request a hash session from the resource manager
    /// and initialize it with the specified [`HashAlgorithm`].
    pub fn new(name: &str, algo: &'static HashAlgorithm) -> Result<Self, Error> {
        let sess = ClientSession::new(name)?;

        // FIXME: Obtain EP immediately with single obtain()
        // This is not possible right now because there is no way to bind the EP to an arbitrary
        // capability selector since those are managed by the EpMng on the server side.
        let crd = sess.obtain(2, |_| {}, |_| Ok(()))?;
        let ep_sel = sess.obtain_obj()?;

        let mut sess = HashSession {
            algo,
            _sess: sess,
            sgate: SendGate::new_bind(crd.start()),
            ep: EP::new_bind(0, ep_sel),
        };
        sess.reset(algo)?;
        Ok(sess)
    }

    /// Returns the hash algorithm that is currently used for this hash session.
    pub fn algo(&self) -> &'static HashAlgorithm {
        self.algo
    }

    /// Returns the [`EP`] that should be configured with [`MemGate`](crate::com::MemGate)s for the
    /// input() and output() operation.
    pub fn ep(&self) -> &EP {
        &self.ep
    }

    /// Reset the state of the hash session (discarding all previous input and
    /// output data) and change the [`HashAlgorithm`].
    pub fn reset(&mut self, algo: &'static HashAlgorithm) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), HashOp::RESET, algo.ty).map(|_| ())?;
        self.algo = algo;
        Ok(())
    }

    /// Input new data into the state of the hash session.
    ///
    /// Before this is called, the [`ep`](HashSession::ep) should be configured with a valid
    /// [`MemGate`](crate::com::MemGate) so that the hash multiplexer can successfully read `len`
    /// bytes with offset `off`.
    pub fn input(&self, off: usize, len: usize) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), HashOp::INPUT, off, len).map(|_| ())
    }

    /// Output new data from the state of the hash session.
    ///
    /// Before this is called, the [`ep`](HashSession::ep) should be configured with a valid
    /// [`MemGate`](crate::com::MemGate) so that the hash multiplexer can successfully write `len`
    /// bytes with offset `off`.
    ///
    /// Note that this operation does not allow output of more bytes than
    /// supported by the current hash algorithm. It is mainly intended for
    /// use with XOFs (extendable output functions) that allow arbitrarily
    /// large output, e.g. as pseudo-random number generator.
    pub fn output(&self, off: usize, len: usize) -> Result<(), Error> {
        if len > self.algo.output_bytes {
            return Err(Error::new(Code::InvArgs));
        }
        send_recv_res!(&self.sgate, RecvGate::def(), HashOp::OUTPUT, off, len).map(|_| ())
    }

    /// Finish the hash for previous [`input`](HashSession::input) data. If successful, the hash is
    /// written to the `result` slice. Note that the ´result` slice must have exactly the size of
    /// `algo().output_bytes`, so this function cannot be used for XOFs (extendable output
    /// functions).
    pub fn finish(&self, result: &mut [u8]) -> Result<(), Error> {
        assert_eq!(result.len(), self.algo.output_bytes);
        send_recv!(self.sgate, RecvGate::def(), HashOp::OUTPUT).and_then(|mut reply| {
            // FIXME: Find a better way to copy out the slice?
            let msg = reply.msg();
            if msg.data.len() != self.algo.output_bytes {
                return Err(Error::new(Code::from(reply.pop::<u32>()?)));
            }

            result.copy_from_slice(&msg.data);
            Ok(())
        })
    }
}

/// A trait for objects that allow directly hashing the contents.
///
/// For example, this is implemented for files. The [`EP`] from the hash
/// multiplexer is delegated to M3FS and M3FS configures the [`EP`] accordingly
/// to let the hash multiplexer read the file contents directly.
pub trait HashInput {
    /// Input a maximum of `len` bytes of this object into the [`HashSession`].
    fn hash_input(&mut self, _sess: &HashSession, _len: usize) -> Result<usize, Error> {
        Err(Error::new(Code::NotSup))
    }
}

/// A trait for objects that allow directly writing hash output data.
///
/// For example, this is implemented for files. The [`EP`] from the hash
/// multiplexer is delegated to M3FS and M3FS configures the [`EP`] accordingly
/// to let the hash multiplexer write the file contents directly.
pub trait HashOutput {
    /// Output a maximum of `len` bytes to this object from the [`HashSession`].
    ///
    /// Note that this operation does not allow output of more bytes than
    /// supported by the current hash algorithm. It is mainly intended for
    /// use with XOFs (extendable output functions) that allow arbitrarily
    /// large output, e.g. as pseudo-random number generator.
    fn hash_output(&mut self, _sess: &HashSession, _len: usize) -> Result<usize, Error> {
        Err(Error::new(Code::NotSup))
    }
}
