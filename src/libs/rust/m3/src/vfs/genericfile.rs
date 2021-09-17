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

use core::cmp;
use core::fmt;

use crate::cap::Selector;
use crate::cell::RefCell;
use crate::col::Vec;
use crate::com::{recv_reply, MemGate, RecvGate, SendGate};
use crate::errors::Error;
use crate::goff;
use crate::int_enum;
use crate::io::{Read, Write};
use crate::kif::{CapRngDesc, CapType, Perm, INVALID_SEL};
use crate::pes::{StateSerializer, VPE};
use crate::rc::Rc;
use crate::serialize::Source;
use crate::session::{ClientSession, MapFlags, Pager};
use crate::time;
use crate::vfs::{
    filetable, Fd, File, FileHandle, FileInfo, Map, OpenFlags, Seek, SeekMode, StatResponse,
};

int_enum! {
    /// The operations for [`GenericFile`].
    pub struct GenFileOp : u64 {
        const STAT      = 0;
        const SEEK      = 1;
        const NEXT_IN   = 2;
        const NEXT_OUT  = 3;
        const COMMIT    = 4;
        const SYNC      = 5;
        const CLOSE     = 6;
        const CLONE     = 7;
        const SET_TMODE = 8;
        const SET_DEST  = 9;
        const SET_SIG   = 10;
    }
}

/// A file implementation for all file-like objects.
///
/// `GenericFile` implements the file protocol and can therefore be used for m3fs files, pipes,
/// virtual terminals, and whatever else provides file-like objects in the future.
pub struct GenericFile {
    fd: Fd,
    flags: OpenFlags,
    sess: ClientSession,
    sgate: SendGate,
    mgate: MemGate,
    goff: usize,
    off: usize,
    pos: usize,
    len: usize,
    writing: bool,
}

impl GenericFile {
    pub(crate) fn new(flags: OpenFlags, sel: Selector) -> Self {
        GenericFile {
            fd: filetable::INV_FD,
            flags,
            sess: ClientSession::new_bind(sel),
            sgate: SendGate::new_bind(sel + 1),
            mgate: MemGate::new_bind(INVALID_SEL),
            goff: 0,
            off: 0,
            pos: 0,
            len: 0,
            writing: false,
        }
    }

    pub(crate) fn unserialize(s: &mut Source) -> FileHandle {
        let flags: u32 = s.pop().unwrap();
        Rc::new(RefCell::new(GenericFile::new(
            OpenFlags::from_bits_truncate(flags),
            s.pop().unwrap(),
        )))
    }

    fn submit(&mut self, force: bool) -> Result<(), Error> {
        if self.pos > 0 && (self.writing || force) {
            send_recv_res!(&self.sgate, RecvGate::def(), GenFileOp::COMMIT, self.pos)?;

            self.goff += self.pos;
            self.pos = 0;
            self.len = 0;
            self.writing = false;
        }
        Ok(())
    }

    fn delegate_ep(&mut self) -> Result<(), Error> {
        if self.mgate.ep().is_none() {
            let ep = self.mgate.activate()?;
            let crd = CapRngDesc::new(CapType::OBJECT, ep.sel(), 1);
            self.sess
                .delegate(crd, |s| s.push_word(GenFileOp::SET_DEST.val), |_| Ok(()))
        }
        else {
            Ok(())
        }
    }
}

impl File for GenericFile {
    fn fd(&self) -> Fd {
        self.fd
    }

    fn set_fd(&mut self, fd: Fd) {
        self.fd = fd;
    }

    fn session(&self) -> Option<Selector> {
        Some(self.sess.sel())
    }

    fn close(&mut self) {
        // submit read/written data
        self.submit(false).ok();

        // revoke EP cap
        if let Some(ep) = self.mgate.ep() {
            VPE::cur()
                .revoke(CapRngDesc::new(CapType::OBJECT, ep.sel(), 1), true)
                .ok();
        }

        // file sessions are not known to our resource manager; thus close them manually
        send_recv_res!(&self.sgate, RecvGate::def(), GenFileOp::CLOSE).ok();
    }

    fn stat(&self) -> Result<FileInfo, Error> {
        send_vmsg!(&self.sgate, RecvGate::def(), GenFileOp::STAT)?;
        let reply = recv_reply(RecvGate::def(), Some(&self.sgate))?;
        let resp = reply.msg().get_data::<StatResponse>();
        FileInfo::from_response(resp)
    }

    fn file_type(&self) -> u8 {
        b'F'
    }

    fn exchange_caps(
        &self,
        vpe: Selector,
        _dels: &mut Vec<Selector>,
        max_sel: &mut Selector,
    ) -> Result<(), Error> {
        let crd = CapRngDesc::new(CapType::OBJECT, self.sess.sel(), 2);
        self.sess
            .obtain_for(vpe, crd, |s| s.push_word(GenFileOp::CLONE.val), |_| Ok(()))?;
        *max_sel = cmp::max(*max_sel, self.sess.sel() + 2);
        Ok(())
    }

    fn serialize(&self, s: &mut StateSerializer) {
        s.push_word(0); // flags
        s.push_word(self.sess.sel());
        s.push_word(0); // id
    }
}

impl Seek for GenericFile {
    fn seek(&mut self, mut off: usize, mut whence: SeekMode) -> Result<usize, Error> {
        self.submit(false)?;

        if whence == SeekMode::CUR {
            off += self.goff + self.pos;
            whence = SeekMode::SET;
        }

        if whence != SeekMode::END
            && self.pos < self.len
            && off > self.goff
            && off < self.goff + self.len
        {
            self.pos = off - self.goff;
            return Ok(off);
        }

        let mut reply = send_recv_res!(&self.sgate, RecvGate::def(), GenFileOp::SEEK, off, whence)?;

        self.goff = reply.pop()?;
        let off: usize = reply.pop()?;
        self.pos = 0;
        self.len = 0;
        Ok(self.goff + off)
    }
}

impl Read for GenericFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.delegate_ep()?;
        self.submit(false)?;

        if self.pos == self.len {
            time::start(0xbbbb);
            let mut reply = send_recv_res!(&self.sgate, RecvGate::def(), GenFileOp::NEXT_IN)?;
            time::stop(0xbbbb);
            self.goff += self.len;
            self.off = reply.pop()?;
            self.len = reply.pop()?;
            self.pos = 0;
        }

        let amount = cmp::min(buf.len(), self.len - self.pos);
        if amount > 0 {
            time::start(0xaaaa);
            self.mgate
                .read(&mut buf[0..amount], (self.off + self.pos) as goff)?;
            time::stop(0xaaaa);
            self.pos += amount;
        }
        self.writing = false;
        Ok(amount)
    }
}

impl Write for GenericFile {
    fn flush(&mut self) -> Result<(), Error> {
        self.submit(false)
    }

    fn sync(&mut self) -> Result<(), Error> {
        self.flush()
            .and_then(|_| send_recv_res!(&self.sgate, RecvGate::def(), GenFileOp::SYNC).map(|_| ()))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.delegate_ep()?;

        if self.pos == self.len {
            time::start(0xbbbb);
            let mut reply = send_recv_res!(&self.sgate, RecvGate::def(), GenFileOp::NEXT_OUT)?;
            time::stop(0xbbbb);
            self.goff += self.len;
            self.off = reply.pop()?;
            self.len = reply.pop()?;
            self.pos = 0;
        }

        let amount = cmp::min(buf.len(), self.len - self.pos);
        if amount > 0 {
            time::start(0xaaaa);
            self.mgate
                .write(&buf[0..amount], (self.off + self.pos) as goff)?;
            time::stop(0xaaaa);
            self.pos += amount;
        }
        self.writing = true;
        Ok(amount)
    }
}

impl Map for GenericFile {
    fn map(
        &self,
        pager: &Pager,
        virt: goff,
        off: usize,
        len: usize,
        prot: Perm,
        flags: MapFlags,
    ) -> Result<(), Error> {
        // TODO maybe check here whether self is a pipe and return an error?
        pager
            .map_ds(virt, len, off, prot, flags, &self.sess)
            .map(|_| ())
    }
}

impl fmt::Debug for GenericFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "GenFile[flags={:?}, sess={}, goff={:#x}, off={:#x}, pos={:#x}, len={:#x}]",
            self.flags,
            self.sess.sel(),
            self.goff,
            self.off,
            self.pos,
            self.len
        )
    }
}
