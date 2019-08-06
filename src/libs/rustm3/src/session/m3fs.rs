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

use cap::Selector;
use cell::RefCell;
use col::Vec;
use com::*;
use core::any::Any;
use core::fmt;
use core::mem::MaybeUninit;
use errors::Error;
use kif;
use rc::{Rc, Weak};
use serialize::Sink;
use session::ClientSession;
use util;
use vfs::{
    FSHandle, FSOperation, FileHandle, FileInfo, FileMode, FileSystem, GenericFile, OpenFlags, VFS,
};
use vpe::VPE;

/// The type of extent ids.
pub type ExtId = u16;

/// Represents a session at m3fs.
pub struct M3FS {
    self_weak: Weak<RefCell<M3FS>>,
    sess: ClientSession,
    sgate: Rc<SendGate>,
}

impl M3FS {
    fn create(sess: ClientSession, sgate: SendGate) -> FSHandle {
        let inst = Rc::new(RefCell::new(M3FS {
            self_weak: Weak::new(),
            sess,
            sgate: Rc::new(sgate),
        }));
        inst.borrow_mut().self_weak = Rc::downgrade(&inst);
        inst
    }

    /// Creates a new session at the m3fs server with given name.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(name: &str) -> Result<FSHandle, Error> {
        let sels = VPE::cur().alloc_sels(2);
        let sess = ClientSession::new_with_sel(name, sels + 1)?;

        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, sels + 0, 1);
        let mut args = kif::syscalls::ExchangeArgs::default();
        sess.obtain_for(VPE::cur().sel(), crd, &mut args)?;
        let sgate = SendGate::new_bind(sels + 0);
        Ok(Self::create(sess, sgate))
    }

    /// Binds a new m3fs-session to selectors `sels`..`sels+1`.
    pub fn new_bind(sels: Selector) -> FSHandle {
        Self::create(
            ClientSession::new_bind(sels + 0),
            SendGate::new_bind(sels + 1),
        )
    }

    /// Returns a reference to the underlying [`ClientSession`]
    pub fn sess(&self) -> &ClientSession {
        &self.sess
    }
}

impl FileSystem for M3FS {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn open(&self, path: &str, mut flags: OpenFlags) -> Result<FileHandle, Error> {
        if flags.contains(OpenFlags::NOSESS) {
            if let Ok((idx, ep)) = VFS::alloc_ep(self.self_weak.upgrade().unwrap()) {
                let mut reply = send_recv_res!(
                    &self.sgate,
                    RecvGate::def(),
                    FSOperation::OPEN_PRIV,
                    path,
                    flags.bits(),
                    idx
                )?;
                let id = reply.pop();
                return Ok(Rc::new(RefCell::new(GenericFile::new_without_sess(
                    flags,
                    id,
                    self.sess.sel(),
                    Some(VPE::cur().sel_ep(ep)),
                    self.sgate.clone(),
                ))));
            }
        }

        flags.remove(OpenFlags::NOSESS);

        let mut args = kif::syscalls::ExchangeArgs {
            count: 1,
            vals: kif::syscalls::ExchangeUnion {
                s: kif::syscalls::ExchangeUnionStr {
                    i: [u64::from(flags.bits()), 0],
                    s: unsafe { MaybeUninit::uninit().assume_init() },
                },
            },
        };

        // copy path
        unsafe {
            for (a, c) in args.vals.s.s.iter_mut().zip(path.bytes()) {
                *a = c as u8;
            }
            args.vals.s.s[path.len()] = b'\0';
        }

        let crd = self.sess.obtain(2, &mut args)?;
        Ok(Rc::new(RefCell::new(GenericFile::new(flags, crd.start()))))
    }

    fn stat(&self, path: &str) -> Result<FileInfo, Error> {
        let mut reply = send_recv_res!(&self.sgate, RecvGate::def(), FSOperation::STAT, path)?;
        Ok(reply.pop())
    }

    fn mkdir(&self, path: &str, mode: FileMode) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), FSOperation::MKDIR, path, mode).map(|_| ())
    }

    fn rmdir(&self, path: &str) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), FSOperation::RMDIR, path).map(|_| ())
    }

    fn link(&self, old_path: &str, new_path: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            FSOperation::LINK,
            old_path,
            new_path
        )
        .map(|_| ())
    }

    fn unlink(&self, path: &str) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), FSOperation::UNLINK, path).map(|_| ())
    }

    fn fs_type(&self) -> u8 {
        b'M'
    }

    fn exchange_caps(
        &self,
        vpe: Selector,
        dels: &mut Vec<Selector>,
        max_sel: &mut Selector,
    ) -> Result<(), Error> {
        dels.push(self.sess.sel());

        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, self.sess.sel() + 1, 1);
        let mut args = kif::syscalls::ExchangeArgs::default();
        self.sess.obtain_for(vpe, crd, &mut args)?;
        *max_sel = util::max(*max_sel, self.sess.sel() + 2);
        Ok(())
    }

    fn serialize(&self, s: &mut VecSink) {
        s.push(&self.sess.sel());
        s.push(&self.sgate.sel());
    }

    fn delegate_eps(&self, first: Selector, count: u32) -> Result<(), Error> {
        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, first, count);
        self.sess.delegate_crd(crd)
    }
}

impl M3FS {
    pub fn unserialize(s: &mut SliceSource) -> FSHandle {
        let sels: Selector = s.pop();
        M3FS::new_bind(sels)
    }
}

impl fmt::Debug for M3FS {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "M3FS[sess={:?}, sgate={:?}]", self.sess, self.sgate)
    }
}
