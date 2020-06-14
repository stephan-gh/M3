/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Init.h>
#include <base/Panic.h>

#include <m3/session/ResMng.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/FileTable.h>
#include <m3/vfs/MountTable.h>
#include <m3/vfs/SerialFile.h>
#include <m3/vfs/VFS.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

namespace m3 {

const size_t VPE::BUF_SIZE    = 4096;
INIT_PRIO_VPE VPE VPE::_self;
VPE *VPE::_self_ptr = &VPE::_self;

VPEArgs::VPEArgs() noexcept
    : _rmng(nullptr),
      _pager(),
      _kmem() {
}

VPEArgs &VPEArgs::pager(Reference<Pager> pager) noexcept {
    _pager = pager;
    return *this;
}

// don't revoke these. they kernel does so on exit
VPE::VPE()
    : ObjCap(VIRTPE, KIF::SEL_VPE, KEEP_CAP),
      _pe(PE::bind(KIF::SEL_PE, PEDesc(env()->pe_desc))),
      _kmem(new KMem(KIF::SEL_KMEM)),
      _next_sel(KIF::FIRST_FREE_SEL),
      _eps_start(),
      _epmng(*this),
      _pager(),
      _resmng(nullptr),
      _ms(),
      _fds(),
      _exec() {
    init_state();
    init_fs();

    // create stdin, stdout and stderr, if not existing
    if(!_fds->exists(STDIN_FD))
        _fds->set(STDIN_FD, Reference<File>(new SerialFile()));
    if(!_fds->exists(STDOUT_FD))
        _fds->set(STDOUT_FD, Reference<File>(new SerialFile()));
    if(!_fds->exists(STDERR_FD))
        _fds->set(STDERR_FD, Reference<File>(new SerialFile()));
}

VPE::VPE(const Reference<class PE> &pe, const String &name, const VPEArgs &args)
    : ObjCap(VIRTPE, VPE::self().alloc_sel()),
      _pe(pe),
      _kmem(args._kmem ? args._kmem : VPE::self().kmem()),
      _next_sel(KIF::FIRST_FREE_SEL),
      _eps_start(),
      _epmng(*this),
      _pager(),
      _resmng(args._rmng),
      _ms(new MountTable()),
      _fds(new FileTable()),
      _exec() {
    // create pager first, to create session and obtain gate cap
    if(_pe->desc().has_virtmem()) {
        if(args._pager)
            _pager = args._pager;
        else if(VPE::self().pager())
            _pager = VPE::self().pager()->create_clone();
        // we need a pager on VM PEs
        else
            throw Exception(Errors::NOT_SUP);
    }

    if(_pager) {
        // now create VPE, which implicitly obtains the gate cap from us
        _eps_start = Syscalls::create_vpe(sel(), _pager->child_sgate().sel(),
                                          _pager->child_rgate().sel(),
                                          name, pe->sel(), _kmem->sel());
        // mark the send gate cap allocated
        _next_sel = Math::max(_pager->child_sgate().sel() + 1, _next_sel);
        // delegate VPE and memory cap to pager
        _pager->delegate_caps(*this);
        // and delegate the pager cap to the VPE
        delegate_obj(_pager->sel());
    }
    else {
        _eps_start = Syscalls::create_vpe(sel(), ObjCap::INVALID, ObjCap::INVALID,
                                          name, pe->sel(), _kmem->sel());
    }
    _next_sel = Math::max(_kmem->sel() + 1, _next_sel);

    if(_resmng == nullptr) {
        _resmng = VPE::self().resmng()->clone(*this, name);
        // ensure that the child's cap space is not further ahead than ours
        // TODO improve that
        VPE::self()._next_sel = Math::max(_next_sel, VPE::self()._next_sel);
    }
    else
        delegate_obj(_resmng->sel());
}

VPE::~VPE() {
    if(this != &_self) {
        try {
            stop();
        }
        catch(...) {
            // ignore
        }
    }
}

void VPE::mounts(const std::unique_ptr<MountTable> &ms) noexcept {
    _ms.reset(new MountTable(*ms.get()));
}

void VPE::obtain_mounts() {
    _ms->delegate(*this);
}

void VPE::fds(const std::unique_ptr<FileTable> &fds) noexcept {
    _fds.reset(new FileTable(*fds.get()));
}

void VPE::obtain_fds() {
    _fds->delegate(*this);
}

void VPE::delegate(const KIF::CapRngDesc &crd, capsel_t dest) {
    Syscalls::exchange(sel(), crd, dest, false);
      _next_sel = Math::max(_next_sel, dest + crd.count());
}

void VPE::obtain(const KIF::CapRngDesc &crd) {
    obtain(crd, VPE::self().alloc_sels(crd.count()));
}

void VPE::obtain(const KIF::CapRngDesc &crd, capsel_t dest) {
    KIF::CapRngDesc own(crd.type(), dest, crd.count());
    Syscalls::exchange(sel(), own, crd.start(), true);
}

void VPE::revoke(const KIF::CapRngDesc &crd, bool delonly) {
    Syscalls::revoke(sel(), crd, !delonly);
}

MemGate VPE::get_mem(goff_t addr, size_t size, int perms) {
    capsel_t nsel = VPE::self().alloc_sel();
    Syscalls::create_mgate(nsel, sel(), addr, size, perms);
    return MemGate::bind(nsel, 0);
}

void VPE::start() {
    Syscalls::vpe_ctrl(sel(), KIF::Syscall::VCTRL_START, 0);
}

void VPE::stop() {
    Syscalls::vpe_ctrl(sel(), KIF::Syscall::VCTRL_STOP, 0);
}

int VPE::wait_async(event_t event) {
    capsel_t _sel;
    const capsel_t sels[] = {sel()};
    return Syscalls::vpe_wait(sels, 1, event, &_sel);
}

int VPE::wait() {
    return wait_async(0);
}

}
