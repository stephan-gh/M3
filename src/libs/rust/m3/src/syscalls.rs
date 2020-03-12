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

//! Contains the system call wrapper functions

use cap::Selector;
use cell::StaticCell;
use com::{RecvGate, SendGate, SliceSink, SliceSource};
use core::mem::MaybeUninit;
use dtu::{DTUIf, EpId, Label, Message, SYSC_SEP};
use errors::Error;
use goff;
use kif::{self, syscalls, CapRngDesc, Perm, INVALID_SEL};
use serialize::Sink;
use util;

static SGATE: StaticCell<SendGate> = StaticCell::new(SendGate::new_def(INVALID_SEL, SYSC_SEP));

struct Reply<R: 'static> {
    msg: &'static Message,
    data: &'static R,
}

impl<R: 'static> Drop for Reply<R> {
    fn drop(&mut self) {
        DTUIf::ack_msg(RecvGate::syscall(), self.msg);
    }
}

fn send_receive<T, R>(msg: *const T) -> Result<Reply<R>, Error> {
    let reply_raw = DTUIf::call(
        &*SGATE,
        msg as *const u8,
        util::size_of::<T>(),
        RecvGate::syscall(),
    )?;

    let reply = reply_raw.get_data::<kif::DefaultReply>();
    if reply.error != 0 {
        DTUIf::ack_msg(RecvGate::syscall(), reply_raw);
        return Err(Error::from(reply.error as u32));
    }

    Ok(Reply {
        msg: reply_raw,
        data: reply_raw.get_data::<R>(),
    })
}

fn send_receive_result<T>(msg: *const T) -> Result<(), Error> {
    send_receive::<T, kif::DefaultReply>(msg).map(|_| ())
}

pub fn send_gate() -> &'static SendGate {
    &*SGATE
}

/// Creates a new service named `name` at selector `dst` for VPE `vpe`. The receive gate `rgate`
/// will be used for service calls from the kernel to the server.
pub fn create_srv(dst: Selector, vpe: Selector, rgate: Selector, name: &str) -> Result<(), Error> {
    #[allow(clippy::uninit_assumed_init)]
    let mut req = syscalls::CreateSrv {
        opcode: syscalls::Operation::CREATE_SRV.val,
        dst_sel: u64::from(dst),
        vpe_sel: u64::from(vpe),
        rgate_sel: u64::from(rgate),
        namelen: name.len() as u64,
        // safety: will be initialized below
        name: unsafe { MaybeUninit::uninit().assume_init() },
    };

    // copy name
    for (a, c) in req.name.iter_mut().zip(name.bytes()) {
        *a = c as u8;
    }

    send_receive_result(&req)
}

/// Creates a new send gate at selector `dst` for receive gate `rgate` using the given label and
/// credit amount.
pub fn create_sgate(
    dst: Selector,
    rgate: Selector,
    label: Label,
    credits: u32,
) -> Result<(), Error> {
    let req = syscalls::CreateSGate {
        opcode: syscalls::Operation::CREATE_SGATE.val,
        dst_sel: u64::from(dst),
        rgate_sel: u64::from(rgate),
        label: u64::from(label),
        credits: u64::from(credits),
    };
    send_receive_result(&req)
}

/// Creates a new receive gate at selector `dst` with a `2^order` bytes receive buffer and
/// `2^msg_order` bytes message slots.
pub fn create_rgate(dst: Selector, order: u32, msgorder: u32) -> Result<(), Error> {
    let req = syscalls::CreateRGate {
        opcode: syscalls::Operation::CREATE_RGATE.val,
        dst_sel: u64::from(dst),
        order: order as u64,
        msgorder: msgorder as u64,
    };
    send_receive_result(&req)
}

/// Creates a new session at selector `dst` for service `srv` and given identifier.
pub fn create_sess(dst: Selector, srv: Selector, ident: u64) -> Result<(), Error> {
    let req = syscalls::CreateSess {
        opcode: syscalls::Operation::CREATE_SESS.val,
        dst_sel: u64::from(dst),
        srv_sel: u64::from(srv),
        ident,
    };
    send_receive_result(&req)
}

/// Creates a new mapping at page `dst` for the given VPE. The syscall maps `pages` pages to the
/// physical memory given by `mgate`, starting at the page `first` within the physical memory using
/// the given permissions.
///
/// Note that the address and size of `mgate` needs to be page aligned.
///
/// # Examples
///
/// The following example allocates 2 pages of physical memory and maps it to page 10 (virtual
/// address 0xA000).
///
/// ```
/// let mem = MemGate::new(0x2000, MemGate::RW).expect("Unable to alloc mem");
/// syscalls::create_map(10, VPE::cur().sel(), mem.sel(), 0, 2, MemGate::RW);
/// ```
pub fn create_map(
    dst: Selector,
    vpe: Selector,
    mgate: Selector,
    first: Selector,
    pages: u32,
    perms: Perm,
) -> Result<(), Error> {
    let req = syscalls::CreateMap {
        opcode: syscalls::Operation::CREATE_MAP.val,
        dst_sel: u64::from(dst),
        vpe_sel: u64::from(vpe),
        mgate_sel: u64::from(mgate),
        first: u64::from(first),
        pages: u64::from(pages),
        perms: u64::from(perms.bits()),
    };
    send_receive_result(&req)
}

/// Creates a new VPE on PE `pe` with given name at the selector range `dst`.
///
/// The argument `sgate` denotes the selector of the `SendGate` to the pager. `kmem` defines the
/// kernel memory to assign to the VPE.
#[allow(clippy::too_many_arguments)]
pub fn create_vpe(
    dst: CapRngDesc,
    pg_sg: Selector,
    pg_rg: Selector,
    name: &str,
    pe: Selector,
    kmem: Selector,
) -> Result<(), Error> {
    #[allow(clippy::uninit_assumed_init)]
    let mut req = syscalls::CreateVPE {
        opcode: syscalls::Operation::CREATE_VPE.val,
        dst_crd: dst.value(),
        pg_sg_sel: u64::from(pg_sg),
        pg_rg_sel: u64::from(pg_rg),
        pe_sel: u64::from(pe),
        kmem_sel: u64::from(kmem),
        namelen: name.len() as u64,
        // safety: will be initialized below
        name: unsafe { MaybeUninit::uninit().assume_init() },
    };

    // copy name
    for (a, c) in req.name.iter_mut().zip(name.bytes()) {
        *a = c as u8;
    }

    send_receive_result(&req)
}

/// Creates a new semaphore at selector `dst` using `value` as the initial value.
pub fn create_sem(dst: Selector, value: u32) -> Result<(), Error> {
    let req = syscalls::CreateSem {
        opcode: syscalls::Operation::CREATE_SEM.val,
        dst_sel: u64::from(dst),
        value: u64::from(value),
    };
    send_receive_result(&req)
}

/// Allocates a new endpoint for the given VPE at selector `dst`. Optionally, it can have `replies`
/// reply slots attached to it (for receive gate activations).
pub fn alloc_ep(dst: Selector, vpe: Selector, epid: EpId, replies: u32) -> Result<EpId, Error> {
    let req = syscalls::AllocEP {
        opcode: syscalls::Operation::ALLOC_EP.val,
        dst_sel: u64::from(dst),
        vpe_sel: u64::from(vpe),
        epid: epid as u64,
        replies: u64::from(replies),
    };

    let reply: Reply<syscalls::AllocEPReply> = send_receive(&req)?;
    Ok(reply.data.ep as EpId)
}

/// Derives a new memory gate for given VPE at selector `dst` based on memory gate `sel`.
///
/// The subset of the region is given by `offset` and `size`, whereas the subset of the permissions
/// are given by `perm`.
pub fn derive_mem(
    vpe: Selector,
    dst: Selector,
    src: Selector,
    offset: goff,
    size: usize,
    perms: Perm,
) -> Result<(), Error> {
    let req = syscalls::DeriveMem {
        opcode: syscalls::Operation::DERIVE_MEM.val,
        vpe_sel: u64::from(vpe),
        dst_sel: u64::from(dst),
        src_sel: u64::from(src),
        offset,
        size: size as u64,
        perms: u64::from(perms.bits()),
    };
    send_receive_result(&req)
}

/// Derives a new kernel memory object at `dst` from `kmem`, transferring `quota` bytes to the new
/// kernel memory object.
pub fn derive_kmem(kmem: Selector, dst: Selector, quota: usize) -> Result<(), Error> {
    let req = syscalls::DeriveKMem {
        opcode: syscalls::Operation::DERIVE_KMEM.val,
        kmem_sel: u64::from(kmem),
        dst_sel: u64::from(dst),
        quota: quota as u64,
    };
    send_receive_result(&req)
}

/// Derives a new PE object at `dst` from `pe`, transferring `eps` EPs to the new PE object.
pub fn derive_pe(pe: Selector, dst: Selector, eps: u32) -> Result<(), Error> {
    let req = syscalls::DerivePE {
        opcode: syscalls::Operation::DERIVE_PE.val,
        pe_sel: u64::from(pe),
        dst_sel: u64::from(dst),
        eps: eps as u64,
    };
    send_receive_result(&req)
}

/// Returns the remaining quota in bytes for the kernel memory object at `kmem`.
pub fn kmem_quota(kmem: Selector) -> Result<usize, Error> {
    let req = syscalls::KMemQuota {
        opcode: syscalls::Operation::KMEM_QUOTA.val,
        kmem_sel: u64::from(kmem),
    };

    let reply: Reply<syscalls::KMemQuotaReply> = send_receive(&req)?;
    Ok(reply.data.amount as usize)
}

/// Returns the remaining quota (free endpoints) for the PE object at `pe`.
pub fn pe_quota(pe: Selector) -> Result<u32, Error> {
    let req = syscalls::PEQuota {
        opcode: syscalls::Operation::PE_QUOTA.val,
        pe_sel: u64::from(pe),
    };

    let reply: Reply<syscalls::PEQuotaReply> = send_receive(&req)?;
    Ok(reply.data.amount as u32)
}

/// Performs the VPE operation `op` with the given VPE.
pub fn vpe_ctrl(vpe: Selector, op: syscalls::VPEOp, arg: u64) -> Result<(), Error> {
    let req = syscalls::VPECtrl {
        opcode: syscalls::Operation::VPE_CTRL.val,
        vpe_sel: u64::from(vpe),
        op: op.val,
        arg,
    };
    send_receive_result(&req)
}

/// Waits until any of the given VPEs exits.
///
/// If `event` is non-zero, the kernel replies immediately and acknowledges the validity of the
/// request and sends an upcall as soon as a VPE exists. Otherwise, the kernel replies only as soon
/// as a VPE exists. In both cases, the kernel returns the selector of the VPE that exited and the
/// exitcode given by the VPE.
pub fn vpe_wait(vpes: &[Selector], event: u64) -> Result<(Selector, i32), Error> {
    #[allow(clippy::uninit_assumed_init)]
    let mut req = syscalls::VPEWait {
        opcode: syscalls::Operation::VPE_WAIT.val,
        event,
        vpe_count: vpes.len() as u64,
        // safety: will be initialized below
        sels: unsafe { MaybeUninit::uninit().assume_init() },
    };
    for (i, sel) in vpes.iter().enumerate() {
        req.sels[i] = u64::from(*sel);
    }

    let reply: Reply<syscalls::VPEWaitReply> = send_receive(&req)?;
    if event != 0 {
        Ok((0, 0))
    }
    else {
        Ok((reply.data.vpe_sel as Selector, reply.data.exitcode as i32))
    }
}

/// Performs the semaphore operation `op` with the given semaphore.
pub fn sem_ctrl(sem: Selector, op: syscalls::SemOp) -> Result<(), Error> {
    let req = syscalls::SemCtrl {
        opcode: syscalls::Operation::SEM_CTRL.val,
        sem_sel: u64::from(sem),
        op: op.val,
    };
    send_receive_result(&req)
}

/// Exchanges capabilities between your VPE and the VPE `vpe`.
///
/// If `obtain` is true, the capabilities `other`..`own.count()` and copied to `own`. If `obtain` is
/// false, the capabilities `own` are copied to `other`..`own.count()`.
pub fn exchange(
    vpe: Selector,
    own: CapRngDesc,
    other: Selector,
    obtain: bool,
) -> Result<(), Error> {
    let req = syscalls::Exchange {
        opcode: syscalls::Operation::EXCHANGE.val,
        vpe_sel: u64::from(vpe),
        own_crd: own.value(),
        other_sel: u64::from(other),
        obtain: u64::from(obtain),
    };
    send_receive_result(&req)
}

/// Delegates the capabilities `crd` of VPE `vpe` via the session `sess` to the server managing the
/// session.
///
/// `pre` and `post` are called before and after the system call, respectively. `pre` is called with
/// `SliceSink`, allowing to pass arguments to the server, whereas `post` is called with
/// `SliceSource`, allowing to get arguments from the server.
pub fn delegate<PRE, POST>(
    vpe: Selector,
    sess: Selector,
    crd: CapRngDesc,
    pre: PRE,
    post: POST,
) -> Result<(), Error>
where
    PRE: Fn(&mut SliceSink),
    POST: FnMut(&mut SliceSource),
{
    exchange_sess(vpe, syscalls::Operation::DELEGATE, sess, crd, pre, post)
}

/// Obtains `crd.count` capabilities via the session `sess` from the server managing the session
/// into `crd` of VPE `vpe`.
///
/// `pre` and `post` are called before and after the system call, respectively. `pre` is called with
/// `SliceSink`, allowing to pass arguments to the server, whereas `post` is called with
/// `SliceSource`, allowing to get arguments from the server.
pub fn obtain<PRE, POST>(
    vpe: Selector,
    sess: Selector,
    crd: CapRngDesc,
    pre: PRE,
    post: POST,
) -> Result<(), Error>
where
    PRE: Fn(&mut SliceSink),
    POST: FnMut(&mut SliceSource),
{
    exchange_sess(vpe, syscalls::Operation::OBTAIN, sess, crd, pre, post)
}

fn exchange_sess<PRE, POST>(
    vpe: Selector,
    op: syscalls::Operation,
    sess: Selector,
    crd: CapRngDesc,
    pre: PRE,
    mut post: POST,
) -> Result<(), Error>
where
    PRE: Fn(&mut SliceSink),
    POST: FnMut(&mut SliceSource),
{
    let mut req = syscalls::ExchangeSess {
        opcode: op.val,
        vpe_sel: u64::from(vpe),
        sess_sel: u64::from(sess),
        crd: crd.value(),
        args: syscalls::ExchangeArgs::default(),
    };

    {
        let mut sink = SliceSink::new(unsafe { &mut req.args.data });
        pre(&mut sink);
        req.args.bytes = sink.size() as u64;
    }

    let reply: Reply<syscalls::ExchangeSessReply> = send_receive(&req)?;

    {
        let words = (reply.data.args.bytes as usize + 7) / 8;
        let mut src = SliceSource::new(unsafe { &reply.data.args.data[..words] });
        post(&mut src);
    }

    Ok(())
}

/// Activates the given gate on given endpoint.
///
/// When activating a receive gate, the address of the receive buffer has to be specified via
/// `addr`.
pub fn activate(ep: Selector, gate: Selector, addr: usize) -> Result<(), Error> {
    let req = syscalls::Activate {
        opcode: syscalls::Operation::ACTIVATE.val,
        ep_sel: u64::from(ep),
        gate_sel: u64::from(gate),
        addr: addr as u64,
    };
    send_receive_result(&req)
}

/// Revokes the given capabilities from given VPE.
///
/// If `own` is true, they are also revoked from the given VPE. Otherwise, only the delegations of
/// the capabilities are revoked.
pub fn revoke(vpe: Selector, crd: CapRngDesc, own: bool) -> Result<(), Error> {
    let req = syscalls::Revoke {
        opcode: syscalls::Operation::REVOKE.val,
        vpe_sel: u64::from(vpe),
        crd: crd.value(),
        own: u64::from(own),
    };
    send_receive_result(&req)
}

/// The noop system call for benchmarking
pub fn noop() -> Result<(), Error> {
    let req = syscalls::Noop {
        opcode: syscalls::Operation::NOOP.val,
    };
    send_receive_result(&req)
}

pub(crate) fn init() {
}

pub(crate) fn reinit() {
}
