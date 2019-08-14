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
use core::mem::MaybeUninit;
use dtu::{DTUIf, EpId, Label, Message, SYSC_REP, SYSC_SEP};
use errors::Error;
use goff;
use kif::{syscalls, CapRngDesc, PEDesc, Perm};
use util;

struct Reply<R: 'static> {
    msg: &'static Message,
    data: &'static R,
}

impl<R: 'static> Drop for Reply<R> {
    fn drop(&mut self) {
        DTUIf::mark_read(SYSC_REP, self.msg);
    }
}

fn send<T>(msg: *const T) -> Result<(), Error> {
    DTUIf::send(
        SYSC_SEP,
        msg as *const u8,
        util::size_of::<T>(),
        0,
        SYSC_REP,
    )
}

fn send_receive<T, R>(msg: *const T) -> Result<Reply<R>, Error> {
    let msg = DTUIf::call(SYSC_SEP, msg as *const u8, util::size_of::<T>(), SYSC_REP)?;
    let data: &[R] = unsafe { &*(&msg.data as *const [u8] as *const [R]) };
    return Ok(Reply {
        msg,
        data: &data[0],
    });
}

fn send_receive_result<T>(msg: *const T) -> Result<(), Error> {
    let reply: Reply<syscalls::DefaultReply> = send_receive(msg)?;

    match reply.data.error {
        0 => Ok(()),
        e => Err(Error::from(e as u32)),
    }
}

/// Creates a new service named `name` at selector `dst` for VPE `vpe`. The receive gate `rgate`
/// will be used for service calls from the kernel to the server.
pub fn create_srv(dst: Selector, vpe: Selector, rgate: Selector, name: &str) -> Result<(), Error> {
    let mut req = syscalls::CreateSrv {
        opcode: syscalls::Operation::CREATE_SRV.val,
        dst_sel: u64::from(dst),
        vpe_sel: u64::from(vpe),
        rgate_sel: u64::from(rgate),
        namelen: name.len() as u64,
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
    credits: u64,
) -> Result<(), Error> {
    let req = syscalls::CreateSGate {
        opcode: syscalls::Operation::CREATE_SGATE.val,
        dst_sel: u64::from(dst),
        rgate_sel: u64::from(rgate),
        label,
        credits,
    };
    send_receive_result(&req)
}

/// Creates a new receive gate at selector `dst` with a `2^order` bytes receive buffer and
/// `2^msg_order` bytes message slots.
pub fn create_rgate(dst: Selector, order: i32, msgorder: i32) -> Result<(), Error> {
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

/// Creates a new VPE with given name at the selector range `dst`.
///
/// The argument `sgate` denotes the selector of the `SendGate` to the pager and `pe` defines the
/// desired PE type for the VPE to run on. The arguments `sep` and `rep` specify the send and
/// receive EPs to use for page fault handling. Finally, `kmem` defines the kernel memory to assign
/// to the VPE.
#[allow(clippy::too_many_arguments)]
pub fn create_vpe(
    dst: CapRngDesc,
    sgate: Selector,
    name: &str,
    pe: PEDesc,
    sep: EpId,
    rep: EpId,
    kmem: Selector,
) -> Result<PEDesc, Error> {
    let mut req = syscalls::CreateVPE {
        opcode: syscalls::Operation::CREATE_VPE.val,
        dst_crd: dst.value(),
        sgate_sel: u64::from(sgate),
        pe: u64::from(pe.value()),
        sep: sep as u64,
        rep: rep as u64,
        kmem_sel: u64::from(kmem),
        namelen: name.len() as u64,
        name: unsafe { MaybeUninit::uninit().assume_init() },
    };

    // copy name
    for (a, c) in req.name.iter_mut().zip(name.bytes()) {
        *a = c as u8;
    }

    let reply: Reply<syscalls::CreateVPEReply> = send_receive(&req)?;
    match reply.data.error {
        0 => Ok(PEDesc::new_from(reply.data.pe as u32)),
        e => Err(Error::from(e as u32)),
    }
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

/// Returns the remaining quota in bytes for the kernel memory object at `kmem`.
pub fn kmem_quota(kmem: Selector) -> Result<usize, Error> {
    let req = syscalls::KMemQuota {
        opcode: syscalls::Operation::KMEM_QUOTA.val,
        kmem_sel: u64::from(kmem),
    };

    let reply: Reply<syscalls::KMemQuotaReply> = send_receive(&req)?;
    match reply.data.error {
        0 => Ok(reply.data.amount as usize),
        e => Err(Error::from(e as u32)),
    }
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
    let mut req = syscalls::VPEWait {
        opcode: syscalls::Operation::VPE_WAIT.val,
        event,
        vpe_count: vpes.len() as u64,
        sels: unsafe { MaybeUninit::uninit().assume_init() },
    };
    for (i, sel) in vpes.iter().enumerate() {
        req.sels[i] = u64::from(*sel);
    }

    let reply: Reply<syscalls::VPEWaitReply> = send_receive(&req)?;
    match { reply.data.error } {
        0 if event != 0 => Ok((0, 0)),
        0 => Ok((reply.data.vpe_sel as Selector, reply.data.exitcode as i32)),
        e => Err(Error::from(e as u32)),
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
/// The arguments are passed to the server to provide further information for the capability
/// delegation and back to the client to provide feedback to the client.
pub fn delegate(
    vpe: Selector,
    sess: Selector,
    crd: CapRngDesc,
    args: &mut syscalls::ExchangeArgs,
) -> Result<(), Error> {
    exchange_sess(vpe, syscalls::Operation::DELEGATE, sess, crd, args)
}

/// Obtains `crd.count` capabilities via the session `sess` from the server managing the session
/// into `crd` of VPE `vpe`.
///
/// The arguments are passed to the server to provide further information for the capability
/// delegation and back to the client to provide feedback to the client.
pub fn obtain(
    vpe: Selector,
    sess: Selector,
    crd: CapRngDesc,
    args: &mut syscalls::ExchangeArgs,
) -> Result<(), Error> {
    exchange_sess(vpe, syscalls::Operation::OBTAIN, sess, crd, args)
}

fn exchange_sess(
    vpe: Selector,
    op: syscalls::Operation,
    sess: Selector,
    crd: CapRngDesc,
    args: &mut syscalls::ExchangeArgs,
) -> Result<(), Error> {
    let req = syscalls::ExchangeSess {
        opcode: op.val,
        vpe_sel: u64::from(vpe),
        sess_sel: u64::from(sess),
        crd: crd.value(),
        args: *args,
    };

    let reply: Reply<syscalls::ExchangeSessReply> = send_receive(&req)?;
    if reply.data.error == 0 {
        *args = reply.data.args;
    }

    match reply.data.error {
        0 => Ok(()),
        e => Err(Error::from(e as u32)),
    }
}

/// Activates the given gate on given endpoint.
///
/// When activating a receive gate, the address of the receive buffer has to be specified via
/// `addr`.
pub fn activate(ep: Selector, gate: Selector, addr: goff) -> Result<(), Error> {
    let req = syscalls::Activate {
        opcode: syscalls::Operation::ACTIVATE.val,
        ep_sel: u64::from(ep),
        gate_sel: u64::from(gate),
        addr,
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

/// Stops the current VPE with given exitcode.
pub fn exit(code: i32) {
    let req = syscalls::VPECtrl {
        opcode: syscalls::Operation::VPE_CTRL.val,
        vpe_sel: 0,
        op: syscalls::VPEOp::STOP.val,
        arg: code as u64,
    };
    send(&req).unwrap();
}
