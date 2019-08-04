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

//! Contains VPE-related abstractions

use arch;
use boxed::Box;
use cap::{CapFlags, Capability, Selector};
use cell::StaticCell;
use col::Vec;
use com::{EpMux, MemGate, SendGate};
use core::fmt;
use core::ops::FnOnce;
use dtu::{EP_COUNT, FIRST_FREE_EP, EpId};
use env;
use errors::{Code, Error};
use goff;
use kif::{CapType, CapRngDesc, INVALID_SEL, PEDesc};
use kif;
use rc::Rc;
use session::{ResMng, Pager};
use syscalls;
use util;
use io::Read;
use vfs::{BufReader, FileRef, OpenFlags, Seek, SeekMode, VFS};
use vfs::{FileTable, Map, MountTable};

/// Represents a [`VPE`] group that is used by the kernel for gang scheduling.
pub struct VPEGroup {
    cap: Capability,
}

impl VPEGroup {
    /// Creates a new VPE group.
    pub fn new() -> Result<Self, Error> {
        let sel = VPE::cur().alloc_sel();

        syscalls::create_vgroup(sel)?;
        Ok(VPEGroup {
            cap: Capability::new(sel, CapFlags::empty()),
        })
    }

    /// Returns the Capability selector of the VPE group.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }
}

/// Represents kernel memory
pub struct KMem {
    cap: Capability,
}

impl KMem {
    pub(crate) fn new(sel: Selector) -> Self {
        KMem {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
        }
    }

    /// Returns the capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the remaining quota of the kernel memory.
    pub fn quota(&self) -> Result<usize, Error> {
        syscalls::kmem_quota(self.sel())
    }

    /// Creates a new kernel memory object and transfers `quota` to the new object.
    pub fn derive(&self, quota: usize) -> Result<Rc<Self>, Error> {
        let sel = VPE::cur().alloc_sel();

        syscalls::derive_kmem(self.sel(), sel, quota)?;
        Ok(Rc::new(KMem {
            cap: Capability::new(sel, CapFlags::empty()),
        }))
    }
}

/// A virtual processing element is used to run an activity on a PE.
pub struct VPE {
    cap: Capability,
    pe: PEDesc,
    mem: MemGate,
    rmng: ResMng,
    next_sel: Selector,
    eps: u64,
    rbufs: arch::rbufs::RBufSpace,
    pager: Option<Pager>,
    kmem: Rc<KMem>,
    files: FileTable,
    mounts: MountTable,
}

/// The arguments for [`VPE`] creations.
pub struct VPEArgs<'n, 'p> {
    name: &'n str,
    pager: Option<&'p str>,
    pe: PEDesc,
    muxable: bool,
    group: Option<VPEGroup>,
    kmem: Option<Rc<KMem>>,
    rmng: Option<ResMng>,
}

/// The mapper trait is used to map the memory of an activity before running it.
pub trait Mapper {
    /// Maps the given file to `virt`..`virt`+`len` with given permissions.
    fn map_file<'l>(&mut self, pager: Option<&'l Pager>, file: &mut BufReader<FileRef>, foff: usize,
                    virt: goff, len: usize, perm: kif::Perm) -> Result<bool, Error>;

    /// Maps anonymous memory to `virt`..`virt`+`len` with given permissions.
    fn map_anon<'l>(&mut self, pager: Option<&'l Pager>,
                    virt: goff, len: usize, perm: kif::Perm) -> Result<bool, Error>;

    /// Initializes the memory at `virt`..`memsize` by loading `fsize` bytes from the given file at
    /// `foff` and zero'ing the remaining space.
    ///
    /// The argument `buf` can be used as a buffer and `mem` refers to the address space of the VPE.
    #[allow(clippy::too_many_arguments)]
    fn init_mem(&self, buf: &mut [u8], mem: &MemGate,
                file: &mut BufReader<FileRef>, foff: usize, fsize: usize,
                virt: goff, memsize: usize) -> Result<(), Error> {
        file.seek(foff, SeekMode::SET)?;

        let mut count = fsize;
        let mut segoff = virt as usize;
        while count > 0 {
            let amount = util::min(count, buf.len());
            let amount = file.read(&mut buf[0..amount])?;

            mem.write(&buf[0..amount], segoff as goff)?;

            count -= amount;
            segoff += amount;
        }

        self.clear_mem(buf, mem, segoff, (memsize - fsize) as usize)
    }

    /// Overwrites `virt`..`virt`+`len` with zeros in the address space given by `mem`.
    ///
    /// The argument `buf` can be used as a buffer.
    fn clear_mem(&self, buf: &mut [u8], mem: &MemGate,
                 mut virt: usize, mut len: usize) -> Result<(), Error> {
        if len == 0 {
            return Ok(())
        }

        for it in buf.iter_mut() {
            *it = 0;
        }

        while len > 0 {
            let amount = util::min(len, buf.len());
            mem.write(&buf[0..amount], virt as goff)?;
            len -= amount;
            virt += amount;
        }

        Ok(())
    }
}

/// The default implementation of the [`Mapper`] trait.
pub struct DefaultMapper {
    has_virtmem: bool,
}

impl DefaultMapper {
    /// Creates a new `DefaultMapper`.
    pub fn new(has_virtmem: bool) -> Self {
        DefaultMapper { has_virtmem }
    }
}

impl Mapper for DefaultMapper {
    fn map_file<'l>(&mut self, pager: Option<&'l Pager>, file: &mut BufReader<FileRef>, foff: usize,
                    virt: goff, len: usize, perm: kif::Perm) -> Result<bool, Error> {
        if let Some(pg) = pager {
            file.get_ref().map(pg, virt, foff, len, perm).map(|_| false)
        }
        else if self.has_virtmem {
            // TODO handle that case
            unimplemented!();
        }
        else {
            Ok(true)
        }
    }
    fn map_anon<'l>(&mut self, pager: Option<&'l Pager>,
                    virt: goff, len: usize, perm: kif::Perm) -> Result<bool, Error> {
        if let Some(pg) = pager {
            pg.map_anon(virt, len, perm).map(|_| false)
        }
        else if self.has_virtmem {
            // TODO handle that case
            unimplemented!();
        }
        else {
            Ok(true)
        }
    }
}

/// Represents an activity that is run on a [`VPE`].
pub trait Activity {
    /// Returns a reference to the VPE.
    fn vpe(&self) -> &VPE;
    /// Returns a mutable reference to the VPE.
    fn vpe_mut(&mut self) -> &mut VPE;

    /// Starts the activity.
    fn start(&self) -> Result<(), Error> {
        syscalls::vpe_ctrl(self.vpe().sel(), kif::syscalls::VPEOp::START, 0).map(|_| ())
    }

    /// Stops the activity.
    fn stop(&self) -> Result<(), Error> {
        syscalls::vpe_ctrl(self.vpe().sel(), kif::syscalls::VPEOp::STOP, 0).map(|_| ())
    }

    /// Waits until the activity exits and returns the error code.
    fn wait(&self) -> Result<i32, Error> {
        syscalls::vpe_wait(&[self.vpe().sel()], 0).map(|r| r.1)
    }

    /// Starts an asynchronous wait for the activity, using the given event for the upcall.
    fn wait_async(&self, event: u64) -> Result<i32, Error> {
        syscalls::vpe_wait(&[self.vpe().sel()], event).map(|r| r.1)
    }
}

/// The activity for [`VPE::run`].
pub struct ClosureActivity {
    vpe: VPE,
    _closure: env::Closure,
}

impl ClosureActivity {
    /// Creates a new `ClosureActivity` for the given VPE and closure.
    pub fn new(vpe: VPE, closure: env::Closure) -> ClosureActivity {
        ClosureActivity {
            vpe,
            _closure: closure,
        }
    }
}

impl Activity for ClosureActivity {
    fn vpe(&self) -> &VPE {
        &self.vpe
    }
    fn vpe_mut(&mut self) -> &mut VPE {
        &mut self.vpe
    }
}

impl Drop for ClosureActivity {
    fn drop(&mut self) {
        self.stop().ok();
        if let Some(ref mut pg) = self.vpe.pager {
            pg.deactivate();
        }
    }
}

/// The activity for [`VPE::exec`].
pub struct ExecActivity {
    vpe: VPE,
    _file: BufReader<FileRef>,
}

impl ExecActivity {
    /// Creates a new `ExecActivity` for the given VPE and executable.
    pub fn new(vpe: VPE, file: BufReader<FileRef>) -> ExecActivity {
        ExecActivity {
            vpe,
            _file: file,
        }
    }
}

impl Activity for ExecActivity {
    fn vpe(&self) -> &VPE {
        &self.vpe
    }
    fn vpe_mut(&mut self) -> &mut VPE {
        &mut self.vpe
    }
}

impl Drop for ExecActivity {
    fn drop(&mut self) {
        self.stop().ok();
        if let Some(ref mut pg) = self.vpe.pager {
            pg.deactivate();
        }
    }
}

impl<'n, 'p> VPEArgs<'n, 'p> {
    /// Creates a new instance of `VPEArgs` using default settings.
    pub fn new(name: &'n str) -> VPEArgs<'n, 'p> {
        VPEArgs {
            name,
            pager: None,
            pe: VPE::cur().pe(),
            muxable: false,
            group: None,
            kmem: None,
            rmng: None,
        }
    }

    /// Sets the resource manager to `rmng`. Otherwise and by default, the resource manager of the
    /// current VPE will be cloned.
    pub fn resmng(mut self, rmng: ResMng) -> Self {
        self.rmng = Some(rmng);
        self
    }

    /// Sets the description of the PE the VPE should be assigned to. By default, the description
    /// of current VPE's PE will be used.
    pub fn pe(mut self, pe: PEDesc) -> Self {
        self.pe = pe;
        self
    }

    /// Sets the name of the pager service. By default, the current pager will be cloned.
    pub fn pager(mut self, pager: &'p str) -> Self {
        self.pager = Some(pager);
        self
    }

    /// Sets whether the assigned PE for the VPE can be shared with other VPEs.
    pub fn muxable(mut self, muxable: bool) -> Self {
        self.muxable = muxable;
        self
    }

    /// Sets the VPE group. By default, the VPE has no group.
    pub fn group(mut self, group: VPEGroup) -> Self {
        self.group = Some(group);
        self
    }

    /// Sets the kernel memory to use for the VPE. By default, the kernel memory of the current VPE
    /// will be used.
    pub fn kmem(mut self, kmem: Rc<KMem>) -> Self {
        self.kmem = Some(kmem);
        self
    }
}

const VMA_RBUF_SIZE: usize  = 64;

static CUR: StaticCell<Option<VPE>> = StaticCell::new(None);

impl VPE {
    fn new_cur() -> Self {
        // currently, the bitmask limits us to 64 endpoints
        const_assert!(EP_COUNT < util::size_of::<u64>() * 8);

        VPE {
            cap: Capability::new(0, CapFlags::KEEP_CAP),
            pe: PEDesc::new_from(0),
            mem: MemGate::new_bind(1),
            rmng: ResMng::new(SendGate::new_bind(0)),    // invalid
            next_sel: kif::FIRST_FREE_SEL,
            eps: 0,
            rbufs: arch::rbufs::RBufSpace::new(),
            pager: None,
            kmem: Rc::new(KMem::new(kif::INVALID_SEL)),
            files: FileTable::default(),
            mounts: MountTable::default(),
        }
    }

    fn init(&mut self) {
        let env = arch::env::get();
        self.pe = env.pe_desc();
        self.next_sel = env.load_nextsel();
        self.rmng = env.load_rmng();
        self.eps = env.load_eps();
        self.rbufs = env.load_rbufs();
        self.pager = env.load_pager();
        self.kmem = env.load_kmem();
        // mounts first; files depend on mounts
        self.mounts = env.load_mounts();
        self.files = env.load_fds();
    }

    /// Returns the currently running `VPE`.
    pub fn cur() -> &'static mut VPE {
        if arch::env::get().has_vpe() {
            arch::env::get().vpe()
        }
        else {
            CUR.get_mut().as_mut().unwrap()
        }
    }

    /// Creates a new `VPE` with given name and default settings. The VPE provides access to the
    /// assigned PE and allows to run an activity on the PE.
    pub fn new(name: &str) -> Result<Self, Error> {
        Self::new_with(VPEArgs::new(name))
    }

    /// Creates a new `VPE` with given arguments. The VPE provides access to the assigned PE and
    /// allows to run an activity on the PE.
    pub fn new_with(args: VPEArgs) -> Result<Self, Error> {
        let sels = VPE::cur().alloc_sels(kif::FIRST_FREE_SEL);

        let mut vpe = VPE {
            cap: Capability::new(sels + 0, CapFlags::empty()),
            pe: args.pe,
            mem: MemGate::new_bind(sels + 1),
            rmng: ResMng::new(SendGate::new_bind(kif::INVALID_SEL)),
            next_sel: kif::FIRST_FREE_SEL,
            eps: 0,
            rbufs: arch::rbufs::RBufSpace::new(),
            pager: None,
            kmem: args.kmem.unwrap_or_else(|| VPE::cur().kmem.clone()),
            files: FileTable::default(),
            mounts: MountTable::default(),
        };

        let rbuf = if args.pe.has_mmu() {
            vpe.alloc_rbuf(VMA_RBUF_SIZE)?
        }
        else {
            0
        };

        let pager = if args.pe.has_virtmem() {
            if let Some(p) = args.pager {
                Some(Pager::new(&mut vpe, rbuf, p)?)
            }
            else if let Some(p) = Self::cur().pager() {
                Some(p.new_clone(&mut vpe, rbuf)?)
            }
            else {
                None
            }
        }
        else {
            None
        };

        let crd = CapRngDesc::new(CapType::OBJECT, vpe.sel(), kif::FIRST_FREE_SEL);
        vpe.pager = if let Some(mut pg) = pager {
            let sgate_sel = pg.child_sgate().sel();

            // now create VPE, which implicitly obtains the gate cap from us
            vpe.pe = syscalls::create_vpe(
                crd, sgate_sel, args.name,
                args.pe, pg.sep(), pg.rep(), args.muxable,
                vpe.kmem.sel(),
                args.group.map_or(INVALID_SEL, |g| g.sel())
            )?;

            // after the VPE creation, we can activate the receive gate
            // note that we do that here in case neither run nor exec is used
            pg.activate(vpe.ep_sel(FIRST_FREE_EP))?;

            // mark the pager caps allocated
            vpe.next_sel = util::max(sgate_sel + 1, vpe.next_sel);
            // now delegate our VPE cap and memory cap to the pager
            pg.delegate_caps(&vpe)?;
            // and delegate the pager cap to the VPE
            vpe.delegate_obj(pg.sel())?;
            Some(pg)
        }
        else {
            vpe.pe = syscalls::create_vpe(
                crd, INVALID_SEL, args.name,
                args.pe, 0, 0, args.muxable,
                vpe.kmem.sel(),
                args.group.map_or(INVALID_SEL, |g| g.sel())
            )?;
            None
        };
        vpe.next_sel = util::max(vpe.kmem.sel() + 1, vpe.next_sel);

        // determine resource manager
        let resmng = if let Some(rmng) = args.rmng {
            vpe.delegate_obj(rmng.sel())?;
            rmng
        }
        else {
            VPE::cur().resmng().clone(&mut vpe, &args.name)?
        };
        vpe.rmng = resmng;
        // ensure that the child's cap space is not further ahead than ours
        // TODO improve that
        VPE::cur().next_sel = util::max(vpe.next_sel, VPE::cur().next_sel);

        Ok(vpe)
    }

    /// Returns the capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }
    /// Returns the description of the PE the VPE has been assigned to.
    pub fn pe(&self) -> PEDesc {
        self.pe
    }
    /// Returns the id of the PE the VPE has been assigned to.
    pub fn pe_id(&self) -> u64 {
        arch::env::get().pe_id()
    }
    /// Returns the `MemGate` that refers to the VPE's address space.
    pub fn mem(&self) -> &MemGate {
        &self.mem
    }

    /// Returns the capability selector for the endpoint with id `ep`.
    pub fn ep_sel(&self, ep: EpId) -> Selector {
        self.sel() + kif::FIRST_EP_SEL + (ep - FIRST_FREE_EP) as Selector
    }
    /// Returns the endpoint id for the given capability selector.
    pub fn sel_ep(&self, sel: Selector) -> EpId {
        (sel - kif::FIRST_EP_SEL) as EpId + FIRST_FREE_EP
    }

    pub(crate) fn rbufs(&mut self) -> &mut arch::rbufs::RBufSpace {
        &mut self.rbufs
    }

    /// Returns a mutable reference to the file table of this VPE.
    pub fn files(&mut self) -> &mut FileTable {
        &mut self.files
    }
    /// Returns a mutable reference to the mount table of this VPE.
    pub fn mounts(&mut self) -> &mut MountTable {
        &mut self.mounts
    }

    /// Returns a reference to the VPE's kernel memory.
    pub fn kmem(&self) -> &Rc<KMem> {
        &self.kmem
    }
    /// Returns a reference to the VPE's resource manager.
    pub fn resmng(&self) -> &ResMng {
        &self.rmng
    }
    /// Returns a reference to the VPE's pager.
    pub fn pager(&self) -> Option<&Pager> {
        self.pager.as_ref()
    }

    /// Allocates a new capability selector and returns it.
    pub fn alloc_sel(&mut self) -> Selector {
        self.alloc_sels(1)
    }
    /// Allocates `count` new and contiguous capability selectors and returns the first one.
    pub fn alloc_sels(&mut self, count: u32) -> Selector {
        self.next_sel += count;
        self.next_sel - count
    }

    /// Allocates and reserves an endpoint, so that it will be no longer considered by the [`EpMux`]
    /// for endpoint multiplexing.
    pub fn alloc_ep(&mut self) -> Result<EpId, Error> {
        for ep in FIRST_FREE_EP..EP_COUNT {
            if self.is_ep_free(ep) {
                self.eps |= 1 << ep;

                // invalidate the EP if necessary
                if self.sel() == 0 {
                    EpMux::get().reserve(ep);
                }

                return Ok(ep)
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    /// Returns true if the given endpoint is still free, that is, not reserved.
    pub fn is_ep_free(&self, ep: EpId) -> bool {
        ep >= FIRST_FREE_EP && (self.eps & (1 << ep)) == 0
    }

    /// Free's the given endpoint, assuming that it has been allocated via [`VPE::alloc_ep`].
    pub fn free_ep(&mut self, ep: EpId) {
        self.eps &= !(1 << ep);
    }

    /// Allocates `size` bytes from the VPE's receive buffer space and returns the address.
    pub fn alloc_rbuf(&mut self, size: usize) -> Result<usize, Error> {
        self.rbufs.alloc(self.pe, size)
    }
    /// Free's the area at `addr` of `size` bytes that had been allocated via [`VPE::alloc_rbuf`].
    pub fn free_rbuf(&mut self, addr: usize, size: usize) {
        self.rbufs.free(addr, size)
    }

    /// Delegates the object capability with selector `sel` of [`VPE::cur`] to `self`.
    pub fn delegate_obj(&mut self, sel: Selector) -> Result<(), Error> {
        self.delegate(CapRngDesc::new(CapType::OBJECT, sel, 1))
    }
    /// Delegates the given capability range of [`VPE::cur`] to `self`.
    pub fn delegate(&mut self, crd: CapRngDesc) -> Result<(), Error> {
        let start = crd.start();
        self.delegate_to(crd, start)
    }
    /// Delegates the given capability range of [`VPE::cur`] to `self` using selectors
    /// `dst`..`dst`+`crd.count()`.
    pub fn delegate_to(&mut self, crd: CapRngDesc, dst: Selector) -> Result<(), Error> {
        syscalls::exchange(self.sel(), crd, dst, false)?;
        self.next_sel = util::max(self.next_sel, dst + crd.count());
        Ok(())
    }

    /// Obtains the object capability with selector `sel` from `self` to [`VPE::cur`].
    pub fn obtain_obj(&mut self, sel: Selector) -> Result<Selector, Error> {
        self.obtain(CapRngDesc::new(CapType::OBJECT, sel, 1))
    }
    /// Obtains the given capability range of `self` to [`VPE::cur`].
    pub fn obtain(&mut self, crd: CapRngDesc) -> Result<Selector, Error> {
        let count = crd.count();
        let start = VPE::cur().alloc_sels(count);
        self.obtain_to(crd, start).map(|_| start)
    }
    /// Obtains the given capability range of `self` to [`VPE::cur`] using selectors
    /// `dst`..`dst`+`crd.count()`.
    pub fn obtain_to(&mut self, crd: CapRngDesc, dst: Selector) -> Result<(), Error> {
        let own = CapRngDesc::new(crd.cap_type(), dst, crd.count());
        syscalls::exchange(self.sel(), own, crd.start(), true)
    }

    /// Revokes the given capability range from `self`.
    ///
    /// If `del_only` is true, only the delegations are revoked, that is, the capability is not
    /// revoked from `self`.
    pub fn revoke(&mut self, crd: CapRngDesc, del_only: bool) -> Result<(), Error> {
        syscalls::revoke(self.sel(), crd, !del_only)
    }

    /// Performs the required capability exchanges to pass the files set for `self` to the VPE.
    ///
    /// Before calling this method, you should adjust the file table of `self` via [`VPE::files`]
    /// by copying files from [`VPE::cur`].
    pub fn obtain_fds(&mut self) -> Result<(), Error> {
        // TODO that's really bad. but how to improve that? :/
        let mut dels = Vec::new();
        self.files.collect_caps(self.sel(), &mut dels, &mut self.next_sel)?;
        for c in dels {
            self.delegate_obj(c)?;
        }
        Ok(())
    }
    /// Performs the required capability exchanges to pass the mounts set for `self` to the VPE.
    ///
    /// Before calling this method, you should adjust the mount table of `self` via [`VPE::mounts`]
    /// by copying mounts from [`VPE::cur`].
    pub fn obtain_mounts(&mut self) -> Result<(), Error> {
        let mut dels = Vec::new();
        self.mounts.collect_caps(self.sel(), &mut dels, &mut self.next_sel)?;
        for c in dels {
            self.delegate_obj(c)?;
        }
        Ok(())
    }

    /// Clones the program running on [`VPE::cur`] to `self` and lets `self` execute the given
    /// function.
    ///
    /// The method returns the `ClosureActivity` on success that can be used to wait for the
    /// functions completeness or to stop it.
    #[cfg(target_os = "none")]
    pub fn run<F>(mut self, func: Box<F>) -> Result<ClosureActivity, Error>
                  where F: FnOnce() -> i32 + Send + 'static {
        use cfg;
        use cpu;

        let first_ep_sel = self.ep_sel(FIRST_FREE_EP);
        if let Some(ref mut pg) = self.pager {
            pg.activate(first_ep_sel)?;
        }

        let env = arch::env::get();
        let mut senv = arch::env::EnvData::default();

        let closure = {
            let mut mapper = DefaultMapper::new(self.pe.has_virtmem());
            let mut loader = arch::loader::Loader::new(
                self.pager.as_ref(), Self::cur().pager().is_some(), &mut mapper, &self.mem
            );

            // copy all regions to child
            senv.set_sp(cpu::get_sp());
            let entry = loader.copy_regions(senv.sp())?;
            senv.set_entry(entry);
            senv.set_heap_size(env.heap_size());
            senv.set_lambda(true);

            // store VPE address to reuse it in the child
            senv.set_vpe(&self);

            // env goes first
            let mut off = cfg::RT_START + util::size_of_val(&senv);

            // create and write closure
            let closure = env::Closure::new(func);
            self.mem.write_obj(&closure, off as goff)?;
            off += util::size_of_val(&closure);

            // write args
            senv.set_argc(env.argc());
            senv.set_argv(loader.write_arguments(&mut off, env::args())?);

            senv.set_pedesc(self.pe());

            // write start env to PE
            self.mem.write_obj(&senv, cfg::RT_START as goff)?;

            closure
        };

        // go!
        let act = ClosureActivity::new(self, closure);
        act.start().map(|_| act)
    }

    /// Clones the program running on [`VPE::cur`] to `self` and lets `self` execute the given
    /// function.
    ///
    /// The method returns the `ClosureActivity` on success that can be used to wait for the
    /// functions completeness or to stop it.
    #[cfg(target_os = "linux")]
    pub fn run<F>(self, func: Box<F>) -> Result<ClosureActivity, Error>
                  where F: FnOnce() -> i32 + Send + 'static {
        use libc;

        let mut closure = env::Closure::new(func);

        let mut p2c = arch::loader::Channel::new()?;
        let mut c2p = arch::loader::Channel::new()?;

        match unsafe { libc::fork() } {
            -1  => {
                Err(Error::new(Code::OutOfMem))
            },

            0   => {
                // wait until the env file has been written by the kernel
                p2c.wait();

                arch::env::reinit();
                arch::env::get().set_vpe(&self);
                ::io::reinit();
                self::reinit();
                ::com::reinit();
                arch::dtu::init();

                c2p.signal();

                let res = closure.call();
                unsafe { libc::exit(res) };
            },

            pid => {
                // let the kernel create the config-file etc. for the given pid
                syscalls::vpe_ctrl(self.sel(), kif::syscalls::VPEOp::START, pid as u64).unwrap();

                p2c.signal();
                // wait until the DTU sockets have been binded
                c2p.wait();

                Ok(ClosureActivity::new(self, closure))
            },
        }
    }

    /// Executes the given program and arguments on `self`.
    ///
    /// The method returns the `ExecActivity` on success that can be used to wait for the
    /// program completeness or to stop it.
    pub fn exec<S: AsRef<str>>(self, args: &[S]) -> Result<ExecActivity, Error> {
        let file = VFS::open(args[0].as_ref(), OpenFlags::RX)?;
        let mut mapper = DefaultMapper::new(self.pe.has_virtmem());
        #[allow(clippy::unnecessary_mut_passed)] // only mutable on gem5
        self.exec_file(&mut mapper, file, args)
    }

    /// Executes the program given as a [`FileRef`] on `self`, using `mapper` to initiate the
    /// address space and `args` as the arguments.
    ///
    /// The method returns the `ExecActivity` on success that can be used to wait for the
    /// program completeness or to stop it.
    #[cfg(target_os = "none")]
    #[allow(unused_mut)]
    pub fn exec_file<S: AsRef<str>>(mut self, mapper: &mut dyn Mapper,
                                    mut file: FileRef, args: &[S]) -> Result<ExecActivity, Error> {
        use cfg;
        use serialize::Sink;
        use com::VecSink;

        let mut file = BufReader::new(file);

        let first_ep_sel = self.ep_sel(FIRST_FREE_EP);
        if let Some(ref mut pg) = self.pager {
            pg.activate(first_ep_sel)?;
        }

        let mut senv = arch::env::EnvData::default();

        {
            let mut loader = arch::loader::Loader::new(
                self.pager.as_ref(), Self::cur().pager().is_some(), mapper, &self.mem
            );

            // load program segments
            senv.set_sp(cfg::STACK_TOP);
            senv.set_entry(loader.load_program(&mut file)?);

            // write args
            let mut off = cfg::RT_START + util::size_of_val(&senv);
            senv.set_argc(args.len());
            senv.set_argv(loader.write_arguments(&mut off, args)?);

            // write file table
            {
                let mut fds = VecSink::default();
                self.files.serialize(&mut fds);
                self.mem.write(fds.words(), off as goff)?;
                senv.set_files(off, fds.size());
                off += fds.size();
            }

            // write mounts table
            {
                let mut mounts = VecSink::default();
                self.mounts.serialize(&mut mounts);
                self.mem.write(mounts.words(), off as goff)?;
                senv.set_mounts(off, mounts.size());
            }

            senv.set_kmem(self.kmem.sel());
            senv.set_rmng(self.rmng.sel());
            senv.set_rbufs(&self.rbufs);
            senv.set_next_sel(self.next_sel);
            senv.set_eps(self.eps);
            senv.set_pedesc(self.pe());

            if let Some(ref pg) = self.pager {
                senv.set_pager(pg);
                senv.set_heap_size(cfg::APP_HEAP_SIZE);
            }
            else {
                senv.set_heap_size(cfg::MOD_HEAP_SIZE);
            }

            // write start env to PE
            self.mem.write_obj(&senv, cfg::RT_START as goff)?;
        }

        // go!
        let act = ExecActivity::new(self, file);
        act.start().map(|_| act)
    }

    /// Executes the program given as a [`FileRef`] on `self`, using `mapper` to initiate the
    /// address space and `args` as the arguments.
    ///
    /// The method returns the `ExecActivity` on success that can be used to wait for the
    /// program completeness or to stop it.
    #[cfg(target_os = "linux")]
    pub fn exec_file<S: AsRef<str>>(self, _mapper: &dyn Mapper,
                                    mut file: FileRef, args: &[S]) -> Result<ExecActivity, Error> {
        use com::VecSink;
        use libc;
        use serialize::Sink;

        let path = arch::loader::copy_file(&mut file)?;

        let mut p2c = arch::loader::Channel::new()?;
        let mut c2p = arch::loader::Channel::new()?;

        match unsafe { libc::fork() } {
            -1  => {
                Err(Error::new(Code::OutOfMem))
            },

            0   => {
                // wait until the env file has been written by the kernel
                p2c.wait();

                let pid = unsafe { libc::getpid() };

                // tell child about fd to notify parent if DTU is ready
                arch::loader::write_env_value(pid, "dturdy", c2p.fds()[1] as u64);

                // write nextsel, eps, rmng, and kmem
                arch::loader::write_env_value(pid, "nextsel", u64::from(self.next_sel));
                arch::loader::write_env_value(pid, "eps", self.eps);
                arch::loader::write_env_value(pid, "rmng", u64::from(self.rmng.sel()));
                arch::loader::write_env_value(pid, "kmem", u64::from(self.kmem.sel()));

                // write rbufs
                let mut rbufs = VecSink::default();
                rbufs.push(&self.rbufs.cur);
                rbufs.push(&self.rbufs.end);
                arch::loader::write_env_file(pid, "rbufs", rbufs.words());

                // write file table
                let mut fds = VecSink::default();
                self.files.serialize(&mut fds);
                arch::loader::write_env_file(pid, "fds", fds.words());

                // write mounts table
                let mut mounts = VecSink::default();
                self.mounts.serialize(&mut mounts);
                arch::loader::write_env_file(pid, "ms", mounts.words());

                arch::loader::exec(args, &path);
            },

            pid => {
                // let the kernel create the config-file etc. for the given pid
                syscalls::vpe_ctrl(self.sel(), kif::syscalls::VPEOp::START, pid as u64).unwrap();

                p2c.signal();
                // wait until the DTU sockets have been binded
                c2p.wait();

                Ok(ExecActivity::new(self, BufReader::new(file)))
            },
        }
    }
}

impl fmt::Debug for VPE {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "VPE[sel: {}, pe: {:?}]", self.sel(), self.pe())
    }
}

pub(crate) fn init() {
    CUR.set(Some(VPE::new_cur()));
    VPE::cur().init();
}

pub(crate) fn reinit() {
    VPE::cur().cap.set_flags(CapFlags::KEEP_CAP);
    VPE::cur().cap = Capability::new(0, CapFlags::KEEP_CAP);
    VPE::cur().mem = MemGate::new_bind(1);
}
