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

use base::kif::{self, syscalls, CapRngDesc, Perm, INVALID_SEL};

use crate::arch;
use crate::cap::Selector;
use crate::cell::{LazyStaticCell, StaticCell};
use crate::com::{RecvGate, SendGate};
use crate::errors::{Code, Error};
use crate::goff;
use crate::mem::MsgBuf;
use crate::serialize::{Sink, Source};
use crate::tcu::{EpId, Label, Message, SYSC_SEP_OFF};

static SGATE: LazyStaticCell<SendGate> = LazyStaticCell::default();
// use a separate message buffer here, because the default buffer could be in use for a message over
// a SendGate, which might have to be activated first using a syscall.
static SYSC_BUF: StaticCell<MsgBuf> = StaticCell::new(MsgBuf::new_initialized());

struct Reply<R: 'static> {
    msg: &'static Message,
    data: &'static R,
}

impl<R: 'static> Drop for Reply<R> {
    fn drop(&mut self) {
        RecvGate::syscall().ack_msg(self.msg).ok();
    }
}

fn send_receive<R>(buf: &MsgBuf) -> Result<Reply<R>, Error> {
    let reply_raw = SGATE.call(buf, RecvGate::syscall())?;

    let reply = reply_raw.get_data::<kif::DefaultReply>();
    let res = Code::from(reply.error as u32);
    if res != Code::None {
        RecvGate::syscall().ack_msg(reply_raw)?;
        return Err(Error::new(res));
    }

    Ok(Reply {
        msg: reply_raw,
        data: reply_raw.get_data::<R>(),
    })
}

fn send_receive_result(buf: &MsgBuf) -> Result<(), Error> {
    send_receive::<kif::DefaultReply>(buf).map(|_| ())
}

#[doc(hidden)]
pub fn send_gate() -> &'static SendGate {
    SGATE.get()
}

/// Creates a new service named `name` at selector `dst`. The receive gate `rgate` will be used for
/// service calls from the kernel to the server.
pub fn create_srv(dst: Selector, rgate: Selector, name: &str, creator: Label) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    syscalls::CreateSrv::fill_msgbuf(buf, dst, rgate, name, creator);
    send_receive_result(&buf)
}

/// Creates a new memory gate at selector `dst` that refers to the address region
/// `addr`..`addr`+`size` in the address space of `vpe`. The `addr` and `size` needs to be page
/// aligned.
pub fn create_mgate(
    dst: Selector,
    vpe: Selector,
    addr: goff,
    size: goff,
    perms: Perm,
) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::CreateMGate {
        opcode: syscalls::Operation::CREATE_MGATE.val,
        dst_sel: dst,
        vpe_sel: vpe,
        addr: addr as u64,
        size: size as u64,
        perms: u64::from(perms.bits()),
    });
    send_receive_result(&buf)
}

/// Creates a new send gate at selector `dst` for receive gate `rgate` using the given label and
/// credit amount.
pub fn create_sgate(
    dst: Selector,
    rgate: Selector,
    label: Label,
    credits: u32,
) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::CreateSGate {
        opcode: syscalls::Operation::CREATE_SGATE.val,
        dst_sel: dst,
        rgate_sel: rgate,
        label: label as u64,
        credits: u64::from(credits),
    });
    send_receive_result(&buf)
}

/// Creates a new receive gate at selector `dst` with a `2^order` bytes receive buffer and
/// `2^msg_order` bytes message slots.
pub fn create_rgate(dst: Selector, order: u32, msgorder: u32) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::CreateRGate {
        opcode: syscalls::Operation::CREATE_RGATE.val,
        dst_sel: dst,
        order: order as u64,
        msgorder: msgorder as u64,
    });
    send_receive_result(&buf)
}

/// Creates a new session at selector `dst` for service `srv` and given identifier. `auto_close`
/// specifies whether the CLOSE message should be sent to the server as soon as all derived session
/// capabilities have been revoked.
pub fn create_sess(
    dst: Selector,
    srv: Selector,
    creator: usize,
    ident: u64,
    auto_close: bool,
) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::CreateSess {
        opcode: syscalls::Operation::CREATE_SESS.val,
        dst_sel: dst,
        srv_sel: srv,
        creator: creator as u64,
        ident,
        auto_close: u64::from(auto_close),
    });
    send_receive_result(&buf)
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
    pages: usize,
    perms: Perm,
) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::CreateMap {
        opcode: syscalls::Operation::CREATE_MAP.val,
        dst_sel: dst,
        vpe_sel: vpe,
        mgate_sel: mgate,
        first,
        pages: pages as u64,
        perms: u64::from(perms.bits()),
    });
    send_receive_result(&buf)
}

/// Creates a new VPE on PE `pe` with given name at the selector range `dst`.
///
/// The argument `sgate` denotes the selector of the `SendGate` to the pager. `kmem` defines the
/// kernel memory to assign to the VPE.
///
/// On success, the function returns the EP id of the first standard EP.
#[allow(clippy::too_many_arguments)]
pub fn create_vpe(
    dst: Selector,
    pg_sg: Selector,
    pg_rg: Selector,
    name: &str,
    pe: Selector,
    kmem: Selector,
) -> Result<EpId, Error> {
    let buf = SYSC_BUF.get_mut();
    syscalls::CreateVPE::fill_msgbuf(buf, dst, pg_sg, pg_rg, name, pe, kmem);

    let reply: Reply<syscalls::CreateVPEReply> = send_receive(&buf)?;
    Ok(reply.data.eps_start as EpId)
}

/// Creates a new semaphore at selector `dst` using `value` as the initial value.
pub fn create_sem(dst: Selector, value: u32) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::CreateSem {
        opcode: syscalls::Operation::CREATE_SEM.val,
        dst_sel: dst,
        value: u64::from(value),
    });
    send_receive_result(&buf)
}

/// Allocates a new endpoint for the given VPE at selector `dst`. Optionally, it can have `replies`
/// reply slots attached to it (for receive gate activations).
pub fn alloc_ep(dst: Selector, vpe: Selector, epid: EpId, replies: u32) -> Result<EpId, Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::AllocEP {
        opcode: syscalls::Operation::ALLOC_EP.val,
        dst_sel: dst,
        vpe_sel: vpe,
        epid: epid as u64,
        replies: u64::from(replies),
    });

    let reply: Reply<syscalls::AllocEPReply> = send_receive(&buf)?;
    Ok(reply.data.ep as EpId)
}

/// Sets the given physical-memory-protection EP to the memory region as defined by the `MemGate`
/// on the given PE.
///
/// The EP has to be between 1 and `crate::tcu::PMEM_PROT_EPS` - 1 and will be overwritten with the
/// new memory region.
pub fn set_pmp(pe: Selector, mgate: Selector, ep: EpId) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::SetPMP {
        opcode: syscalls::Operation::SET_PMP.val,
        pe_sel: pe,
        mgate_sel: mgate,
        epid: u64::from(ep),
    });
    send_receive_result(&buf)
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
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::DeriveMem {
        opcode: syscalls::Operation::DERIVE_MEM.val,
        vpe_sel: vpe,
        dst_sel: dst,
        src_sel: src,
        offset,
        size: size as u64,
        perms: u64::from(perms.bits()),
    });
    send_receive_result(&buf)
}

/// Derives a new kernel memory object at `dst` from `kmem`, transferring `quota` bytes to the new
/// kernel memory object.
pub fn derive_kmem(kmem: Selector, dst: Selector, quota: usize) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::DeriveKMem {
        opcode: syscalls::Operation::DERIVE_KMEM.val,
        kmem_sel: kmem,
        dst_sel: dst,
        quota: quota as u64,
    });
    send_receive_result(&buf)
}

/// Derives a new PE object at `dst` from `pe`, transferring `eps` EPs to the new PE object.
pub fn derive_pe(pe: Selector, dst: Selector, eps: u32) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::DerivePE {
        opcode: syscalls::Operation::DERIVE_PE.val,
        pe_sel: pe,
        dst_sel: dst,
        eps: eps as u64,
    });
    send_receive_result(&buf)
}

/// Derives a new service object at `dst` + 0 and a send gate to create sessions at `dst` + 1 from
/// existing service `srv`, transferring `sessions` sessions to the new service object.
/// A non-error reply just acknowledges that the request has been sent to the service. Upon the
/// completion of the request, you will receive an upcall containing `event`.
pub fn derive_srv(srv: Selector, dst: CapRngDesc, sessions: u32, event: u64) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::DeriveSrv {
        opcode: syscalls::Operation::DERIVE_SRV.val,
        dst_sel: dst.start(),
        srv_sel: srv,
        sessions: sessions as u64,
        event,
    });
    send_receive_result(&buf)
}

/// Obtains the session capability from service `srv` with session id `sid` to the given VPE.
pub fn get_sess(srv: Selector, vpe: Selector, dst: Selector, sid: Label) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::GetSession {
        opcode: syscalls::Operation::GET_SESS.val,
        dst_sel: dst,
        srv_sel: srv,
        vpe_sel: vpe,
        sid: sid as u64,
    });
    send_receive_result(&buf)
}

/// Returns the remaining quota in bytes for the kernel memory object at `kmem`.
pub fn kmem_quota(kmem: Selector) -> Result<usize, Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::KMemQuota {
        opcode: syscalls::Operation::KMEM_QUOTA.val,
        kmem_sel: kmem,
    });

    let reply: Reply<syscalls::KMemQuotaReply> = send_receive(&buf)?;
    Ok(reply.data.amount as usize)
}

/// Returns the remaining quota (free endpoints) for the PE object at `pe`.
pub fn pe_quota(pe: Selector) -> Result<u32, Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::PEQuota {
        opcode: syscalls::Operation::PE_QUOTA.val,
        pe_sel: pe,
    });

    let reply: Reply<syscalls::PEQuotaReply> = send_receive(&buf)?;
    Ok(reply.data.amount as u32)
}

/// Performs the VPE operation `op` with the given VPE.
pub fn vpe_ctrl(vpe: Selector, op: syscalls::VPEOp, arg: u64) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::VPECtrl {
        opcode: syscalls::Operation::VPE_CTRL.val,
        vpe_sel: vpe,
        op: op.val,
        arg,
    });

    if vpe == kif::SEL_VPE && op == syscalls::VPEOp::STOP {
        SGATE.send(&buf, RecvGate::syscall())
    }
    else {
        send_receive_result(&buf)
    }
}

/// Waits until any of the given VPEs exits.
///
/// If `event` is non-zero, the kernel replies immediately and acknowledges the validity of the
/// request and sends an upcall as soon as a VPE exists. Otherwise, the kernel replies only as soon
/// as a VPE exists. In both cases, the kernel returns the selector of the VPE that exited and the
/// exitcode given by the VPE.
pub fn vpe_wait(vpes: &[Selector], event: u64) -> Result<(Selector, i32), Error> {
    let buf = SYSC_BUF.get_mut();
    syscalls::VPEWait::fill_msgbuf(buf, vpes, event);

    let reply: Reply<syscalls::VPEWaitReply> = send_receive(&buf)?;
    if event != 0 {
        Ok((0, 0))
    }
    else {
        Ok((reply.data.vpe_sel as Selector, reply.data.exitcode as i32))
    }
}

/// Performs the semaphore operation `op` with the given semaphore.
pub fn sem_ctrl(sem: Selector, op: syscalls::SemOp) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::SemCtrl {
        opcode: syscalls::Operation::SEM_CTRL.val,
        sem_sel: sem,
        op: op.val,
    });
    send_receive_result(&buf)
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
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::Exchange {
        opcode: syscalls::Operation::EXCHANGE.val,
        vpe_sel: vpe,
        own_caps: own.raw(),
        other_sel: other,
        obtain: u64::from(obtain),
    });
    send_receive_result(&buf)
}

/// Delegates the capabilities `crd` of VPE `vpe` via the session `sess` to the server managing the
/// session.
///
/// `pre` and `post` are called before and after the system call, respectively. `pre` is called with
/// [`Sink`], allowing to pass arguments to the server, whereas `post` is called with [`Source`],
/// allowing to get arguments from the server.
pub fn delegate<PRE, POST>(
    vpe: Selector,
    sess: Selector,
    crd: CapRngDesc,
    pre: PRE,
    post: POST,
) -> Result<(), Error>
where
    PRE: Fn(&mut Sink),
    POST: FnMut(&mut Source) -> Result<(), Error>,
{
    exchange_sess(vpe, syscalls::Operation::DELEGATE, sess, crd, pre, post)
}

/// Obtains `crd.count` capabilities via the session `sess` from the server managing the session
/// into `crd` of VPE `vpe`.
///
/// `pre` and `post` are called before and after the system call, respectively. `pre` is called with
/// [`Sink`], allowing to pass arguments to the server, whereas `post` is called with [`Source`],
/// allowing to get arguments from the server.
pub fn obtain<PRE, POST>(
    vpe: Selector,
    sess: Selector,
    crd: CapRngDesc,
    pre: PRE,
    post: POST,
) -> Result<(), Error>
where
    PRE: Fn(&mut Sink),
    POST: FnMut(&mut Source) -> Result<(), Error>,
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
    PRE: Fn(&mut Sink),
    POST: FnMut(&mut Source) -> Result<(), Error>,
{
    let buf = SYSC_BUF.get_mut();
    let req = buf.set(syscalls::ExchangeSess {
        opcode: op.val,
        vpe_sel: vpe,
        sess_sel: sess,
        caps: crd.raw(),
        args: syscalls::ExchangeArgs::default(),
    });

    {
        let mut sink = Sink::new(&mut req.args.data);
        pre(&mut sink);
        req.args.bytes = sink.size() as u64;
    }

    let reply: Reply<syscalls::ExchangeSessReply> = send_receive(&buf)?;

    {
        let words = (reply.data.args.bytes as usize + 7) / 8;
        let mut src = Source::new(&reply.data.args.data[..words]);
        post(&mut src)?;
    }

    Ok(())
}

/// Activates the given gate on given endpoint.
///
/// When activating a receive gate, the physical memory of the receive buffer and its offset needs
/// to be specified via `rbuf_mem` and `rbuf_off`.
pub fn activate(
    ep: Selector,
    gate: Selector,
    rbuf_mem: Selector,
    rbuf_off: usize,
) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::Activate {
        opcode: syscalls::Operation::ACTIVATE.val,
        ep_sel: ep,
        gate_sel: gate,
        rbuf_mem,
        rbuf_off: rbuf_off as u64,
    });
    send_receive_result(&buf)
}

/// Revokes the given capabilities from given VPE.
///
/// If `own` is true, they are also revoked from the given VPE. Otherwise, only the delegations of
/// the capabilities are revoked.
pub fn revoke(vpe: Selector, crd: CapRngDesc, own: bool) -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::Revoke {
        opcode: syscalls::Operation::REVOKE.val,
        vpe_sel: vpe,
        caps: crd.raw(),
        own: u64::from(own),
    });
    send_receive_result(&buf)
}

/// The reset stats system call for benchmarking
///
/// Resets the statistics for all VPEs in the system
pub fn reset_stats() -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::ResetStats {
        opcode: syscalls::Operation::RESET_STATS.val,
    });
    send_receive_result(&buf)
}

/// The noop system call for benchmarking
pub fn noop() -> Result<(), Error> {
    let buf = SYSC_BUF.get_mut();
    buf.set(syscalls::Noop {
        opcode: syscalls::Operation::NOOP.val,
    });
    send_receive_result(&buf)
}

pub(crate) fn init() {
    let env = arch::env::get();
    SGATE.set(SendGate::new_def(
        INVALID_SEL,
        env.first_std_ep() + SYSC_SEP_OFF,
    ));
}

pub(crate) fn reinit() {
    init();
}
