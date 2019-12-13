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

//! Contains the VPE abstraction

use arch;
use boxed::Box;
use cap::{CapFlags, Capability, Selector};
use cell::StaticCell;
use col::Vec;
use com::{EpMng, MemGate, SendGate};
use core::cmp;
use core::fmt;
use core::ops::FnOnce;
use env;
use errors::Error;
use kif;
use kif::{CapRngDesc, CapType, PEDesc, INVALID_SEL};
use pes::{ClosureActivity, DefaultMapper, ExecActivity, KMem, Mapper, PE};
use rc::Rc;
use session::{Pager, ResMng};
use syscalls;
use vfs::{BufReader, FileRef, OpenFlags, VFS};
use vfs::{FileTable, MountTable};

/// A virtual processing element is used to run an activity on a PE.
pub struct VPE {
    cap: Capability,
    rmng: ResMng, // close the connection resource manager at last
    pe: Rc<PE>,
    kmem: Rc<KMem>,
    mem: MemGate,
    next_sel: Selector,
    epmng: EpMng,
    rbufs: arch::rbufs::RBufSpace,
    pager: Option<Pager>,
    files: FileTable,
    mounts: MountTable,
}

/// The arguments for [`VPE`] creations.
pub struct VPEArgs<'n, 'p> {
    name: &'n str,
    pager: Option<&'p str>,
    kmem: Option<Rc<KMem>>,
    rmng: Option<ResMng>,
}

impl<'n, 'p> VPEArgs<'n, 'p> {
    /// Creates a new instance of `VPEArgs` using default settings.
    pub fn new(name: &'n str) -> VPEArgs<'n, 'p> {
        VPEArgs {
            name,
            pager: None,
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

    /// Sets the name of the pager service. By default, the current pager will be cloned.
    pub fn pager(mut self, pager: &'p str) -> Self {
        self.pager = Some(pager);
        self
    }

    /// Sets the kernel memory to use for the VPE. By default, the kernel memory of the current VPE
    /// will be used.
    pub fn kmem(mut self, kmem: Rc<KMem>) -> Self {
        self.kmem = Some(kmem);
        self
    }
}

static CUR: StaticCell<Option<VPE>> = StaticCell::new(None);

impl VPE {
    fn new_cur() -> Self {
        VPE {
            cap: Capability::new(kif::SEL_VPE, CapFlags::KEEP_CAP),
            pe: Rc::new(PE::new_bind(PEDesc::new_from(0), kif::SEL_PE)),
            mem: MemGate::new_bind(kif::SEL_MEM),
            rmng: ResMng::new(SendGate::new_bind(kif::INVALID_SEL)), // invalid
            next_sel: kif::FIRST_FREE_SEL,
            epmng: EpMng::new(),
            rbufs: arch::rbufs::RBufSpace::new(),
            pager: None,
            kmem: Rc::new(KMem::new(kif::SEL_KMEM)),
            files: FileTable::default(),
            mounts: MountTable::default(),
        }
    }

    fn init(&mut self) {
        let env = arch::env::get();
        self.pe = Rc::new(PE::new_bind(env.pe_desc(), kif::SEL_PE));
        self.next_sel = env.load_nextsel();
        self.rmng = env.load_rmng();
        self.rbufs = env.load_rbufs();
        self.pager = env.load_pager();
        // mounts first; files depend on mounts
        self.mounts = env.load_mounts();
        self.files = env.load_fds();
        self.epmng.reset();
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

    /// Creates a new `VPE` on PE `pe` with given name and default settings. The VPE provides access
    /// to the PE and allows to run an activity on the PE.
    pub fn new(pe: Rc<PE>, name: &str) -> Result<Self, Error> {
        Self::new_with(pe, VPEArgs::new(name))
    }

    /// Creates a new `VPE` on PE `pe` with given arguments. The VPE provides access to the PE and
    /// allows to run an activity on the PE.
    pub fn new_with(pe: Rc<PE>, args: VPEArgs) -> Result<Self, Error> {
        let sels = VPE::cur().alloc_sels(kif::FIRST_FREE_SEL);

        let mut vpe = VPE {
            cap: Capability::new(sels + kif::SEL_VPE, CapFlags::empty()),
            pe: pe.clone(),
            kmem: args.kmem.unwrap_or_else(|| VPE::cur().kmem.clone()),
            mem: MemGate::new_bind(sels + kif::SEL_MEM),
            rmng: ResMng::new(SendGate::new_bind(kif::INVALID_SEL)),
            next_sel: kif::FIRST_FREE_SEL,
            epmng: EpMng::new(),
            rbufs: arch::rbufs::RBufSpace::new(),
            pager: None,
            files: FileTable::default(),
            mounts: MountTable::default(),
        };

        let pager = if vpe.pe.desc().has_virtmem() {
            if let Some(p) = args.pager {
                Some(Pager::new(p)?)
            }
            else if let Some(p) = Self::cur().pager() {
                Some(p.new_clone()?)
            }
            else {
                None
            }
        }
        else {
            None
        };

        let crd = CapRngDesc::new(CapType::OBJECT, sels, kif::FIRST_FREE_SEL);
        vpe.pager = if let Some(mut pg) = pager {
            let sgate_sel = pg.child_sgate().sel();
            let rgate_sel = pg.child_rgate().sel();

            // now create VPE, which implicitly obtains the gate cap from us
            syscalls::create_vpe(
                crd,
                sgate_sel,
                rgate_sel,
                args.name,
                pe.sel(),
                vpe.kmem.sel(),
            )?;

            // mark the pager caps allocated
            vpe.next_sel = cmp::max(sgate_sel + 1, vpe.next_sel);
            // now delegate our VPE cap and memory cap to the pager
            pg.delegate_caps(&vpe)?;
            // and delegate the pager cap to the VPE
            vpe.delegate_obj(pg.sel())?;
            Some(pg)
        }
        else {
            syscalls::create_vpe(
                crd,
                INVALID_SEL,
                INVALID_SEL,
                args.name,
                pe.sel(),
                vpe.kmem.sel(),
            )?;
            None
        };
        vpe.next_sel = cmp::max(vpe.kmem.sel() + 1, vpe.next_sel);

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
        VPE::cur().next_sel = cmp::max(vpe.next_sel, VPE::cur().next_sel);

        Ok(vpe)
    }

    /// Returns the capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the description of the PE the VPE has been assigned to.
    pub fn pe(&self) -> &Rc<PE> {
        &self.pe
    }

    /// Returns the description of the PE the VPE has been assigned to.
    pub fn pe_desc(&self) -> PEDesc {
        self.pe.desc()
    }

    /// Returns the id of the PE the VPE has been assigned to.
    pub fn pe_id(&self) -> u32 {
        arch::env::get().pe_id()
    }

    /// Returns the `MemGate` that refers to the VPE's address space.
    pub fn mem(&self) -> &MemGate {
        &self.mem
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

    /// Returns a mutable reference to the endpoint manager
    pub fn epmng(&mut self) -> &mut EpMng {
        &mut self.epmng
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

    /// Allocates `size` bytes from the VPE's receive buffer space and returns the address.
    pub fn alloc_rbuf(&mut self, size: usize) -> Result<usize, Error> {
        self.rbufs.alloc(self.pe_desc(), size)
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
        self.next_sel = cmp::max(self.next_sel, dst + crd.count());
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
        self.files
            .collect_caps(self.sel(), &mut dels, &mut self.next_sel)?;
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
        self.mounts
            .collect_caps(self.sel(), &mut dels, &mut self.next_sel)?;
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
    pub fn run<F>(self, func: Box<F>) -> Result<ClosureActivity, Error>
    where
        F: FnOnce() -> i32 + Send + 'static,
    {
        use cfg;
        use cpu;
        use goff;
        use pes::Activity;
        use util;

        let env = arch::env::get();
        let mut senv = arch::env::EnvData::default();

        let closure = {
            let mut mapper = DefaultMapper::new(self.pe_desc().has_virtmem());
            let mut loader = arch::loader::Loader::new(
                self.pager.as_ref(),
                Self::cur().pager().is_some(),
                &mut mapper,
                &self.mem,
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
            let mut off = cfg::ENV_START + util::size_of_val(&senv);

            // create and write closure
            let closure = env::Closure::new(func);
            self.mem.write_obj(&closure, off as goff)?;
            off += util::size_of_val(&closure);

            // write args
            senv.set_argc(env.argc());
            senv.set_argv(loader.write_arguments(&mut off, env::args())?);

            senv.set_shared(arch::env::get().shared());
            senv.set_pedesc(self.pe_desc());

            // write start env to PE
            self.mem.write_obj(&senv, cfg::ENV_START as goff)?;

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
    where
        F: FnOnce() -> i32 + Send + 'static,
    {
        use errors::Code;
        use libc;
        use pes;

        let mut closure = env::Closure::new(func);

        let mut p2c = arch::loader::Channel::new()?;
        let mut c2p = arch::loader::Channel::new()?;

        match unsafe { libc::fork() } {
            -1 => Err(Error::new(Code::OutOfMem)),

            0 => {
                // wait until the env file has been written by the kernel
                p2c.wait();

                arch::env::reinit();
                arch::env::get().set_vpe(&self);
                ::io::reinit();
                pes::reinit();
                syscalls::reinit();
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
        let mut mapper = DefaultMapper::new(self.pe_desc().has_virtmem());
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
    pub fn exec_file<S: AsRef<str>>(
        mut self,
        mapper: &mut dyn Mapper,
        mut file: FileRef,
        args: &[S],
    ) -> Result<ExecActivity, Error> {
        use cfg;
        use com::VecSink;
        use goff;
        use pes::Activity;
        use serialize::Sink;
        use util;

        let mut file = BufReader::new(file);

        let mut senv = arch::env::EnvData::default();

        {
            let mut loader = arch::loader::Loader::new(
                self.pager.as_ref(),
                Self::cur().pager().is_some(),
                mapper,
                &self.mem,
            );

            // load program segments
            senv.set_sp(cfg::STACK_TOP);
            senv.set_entry(loader.load_program(&mut file)?);

            // write args
            let mut off = cfg::ENV_START + util::size_of_val(&senv);
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

            senv.set_rmng(self.rmng.sel());
            senv.set_rbufs(&self.rbufs);
            senv.set_next_sel(self.next_sel);
            senv.set_shared(arch::env::get().shared());
            senv.set_pedesc(self.pe_desc());

            if let Some(ref pg) = self.pager {
                senv.set_pager(pg);
                senv.set_heap_size(cfg::APP_HEAP_SIZE);
            }
            else {
                senv.set_heap_size(cfg::MOD_HEAP_SIZE);
            }

            // write start env to PE
            self.mem.write_obj(&senv, cfg::ENV_START as goff)?;
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
    pub fn exec_file<S: AsRef<str>>(
        self,
        _mapper: &dyn Mapper,
        mut file: FileRef,
        args: &[S],
    ) -> Result<ExecActivity, Error> {
        use com::VecSink;
        use errors::Code;
        use libc;
        use serialize::Sink;

        let path = arch::loader::copy_file(&mut file)?;

        let mut p2c = arch::loader::Channel::new()?;
        let mut c2p = arch::loader::Channel::new()?;

        match unsafe { libc::fork() } {
            -1 => Err(Error::new(Code::OutOfMem)),

            0 => {
                // wait until the env file has been written by the kernel
                p2c.wait();

                let pid = unsafe { libc::getpid() };

                // tell child about fd to notify parent if DTU is ready
                arch::loader::write_env_value(pid, "dturdy", c2p.fds()[1] as u64);

                // write nextsel, eps, rmng, and kmem
                arch::loader::write_env_value(pid, "nextsel", u64::from(self.next_sel));
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
    VPE::cur().cap = Capability::new(kif::SEL_VPE, CapFlags::KEEP_CAP);
    // be careful not to destruct the object
    VPE::cur().pe.set_sel(kif::SEL_PE);
    VPE::cur().mem = MemGate::new_bind(kif::SEL_MEM);
    VPE::cur().kmem = Rc::new(KMem::new(kif::SEL_KMEM));
    VPE::cur().epmng().reset();
}
