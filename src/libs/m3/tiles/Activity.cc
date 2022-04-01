/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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
#include <m3/tiles/Activity.h>

namespace m3 {

const size_t Activity::BUF_SIZE    = 4096;
INIT_PRIO_ACT Activity Activity::_self;

ActivityArgs::ActivityArgs() noexcept
    : _rmng(nullptr),
      _pager(),
      _kmem() {
}

ActivityArgs &ActivityArgs::pager(Reference<Pager> pager) noexcept {
    _pager = pager;
    return *this;
}

// don't revoke these. they kernel does so on exit
Activity::Activity()
    : ObjCap(ACTIVITY, KIF::SEL_ACT, KEEP_CAP),
      _id(),
      _tile(Tile::bind(KIF::SEL_TILE, TileDesc(env()->tile_desc))),
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

Activity::Activity(const Reference<class Tile> &tile, const String &name, const ActivityArgs &args)
    : ObjCap(ACTIVITY, Activity::self().alloc_sels(3)),
      _id(),
      _tile(tile),
      _kmem(args._kmem ? args._kmem : Activity::self().kmem()),
      _next_sel(KIF::FIRST_FREE_SEL),
      _eps_start(),
      _epmng(*this),
      _pager(),
      _resmng(args._rmng),
      _ms(new MountTable()),
      _fds(new FileTable()),
      _exec() {
    // create pager first, to create session and obtain gate cap
    if(_tile->desc().has_virtmem()) {
        if(args._pager)
            _pager = args._pager;
        else if(Activity::self().pager())
            _pager = Activity::self().pager()->create_clone();
        // we need a pager on VM tiles
        else
            throw Exception(Errors::NOT_SUP);
    }

    if(_pager) {
        // now create activity, which implicitly obtains the gate cap from us
        _eps_start = Syscalls::create_activity(sel(), name, tile->sel(), _kmem->sel(), &_id);
        // delegate activity cap to pager
        _pager->init(*this);
    }
    else
        _eps_start = Syscalls::create_activity(sel(), name, tile->sel(), _kmem->sel(), &_id);
    _next_sel = Math::max(_kmem->sel() + 1, _next_sel);

    if(_resmng == nullptr) {
        _resmng = Activity::self().resmng()->clone(*this, name);
        // ensure that the child's cap space is not further ahead than ours
        // TODO improve that
        Activity::self()._next_sel = Math::max(_next_sel, Activity::self()._next_sel);
    }
    else
        delegate_obj(_resmng->sel());
}

Activity::~Activity() {
    if(this != &_self) {
        try {
            stop();
        }
        catch(...) {
            // ignore
        }
    }
}

void Activity::obtain_mounts() {
    _ms->delegate(*this);
}

void Activity::obtain_fds() {
    _fds->delegate(*this);
}

void Activity::delegate(const KIF::CapRngDesc &crd, capsel_t dest) {
    Syscalls::exchange(sel(), crd, dest, false);
    _next_sel = Math::max(_next_sel, dest + crd.count());
}

void Activity::obtain(const KIF::CapRngDesc &crd) {
    obtain(crd, Activity::self().alloc_sels(crd.count()));
}

void Activity::obtain(const KIF::CapRngDesc &crd, capsel_t dest) {
    KIF::CapRngDesc own(crd.type(), dest, crd.count());
    Syscalls::exchange(sel(), own, crd.start(), true);
}

void Activity::revoke(const KIF::CapRngDesc &crd, bool delonly) {
    Syscalls::revoke(sel(), crd, !delonly);
}

MemGate Activity::get_mem(goff_t addr, size_t size, int perms) {
    capsel_t nsel = Activity::self().alloc_sel();
    Syscalls::create_mgate(nsel, sel(), addr, size, perms);
    return MemGate::bind(nsel, 0);
}

void Activity::start() {
    Syscalls::activity_ctrl(sel(), KIF::Syscall::VCTRL_START, 0);
}

void Activity::stop() {
    Syscalls::activity_ctrl(sel(), KIF::Syscall::VCTRL_STOP, 0);
}

int Activity::wait_async(event_t event) {
    capsel_t _sel;
    const capsel_t sels[] = {sel()};
    return Syscalls::activity_wait(sels, 1, event, &_sel);
}

int Activity::wait() {
    return wait_async(0);
}

void Activity::exec(int argc, const char **argv) {
    do_exec(argc, argv, 0);
}

}
