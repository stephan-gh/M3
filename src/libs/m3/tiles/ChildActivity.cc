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

#include <m3/Syscalls.h>
#include <m3/session/ResMng.h>
#include <m3/stream/FStream.h>
#include <m3/tiles/ChildActivity.h>
#include <m3/tiles/OwnActivity.h>
#include <m3/vfs/File.h>
#include <m3/vfs/FileTable.h>

namespace m3 {

const size_t ChildActivity::BUF_SIZE = 4096;

ActivityArgs::ActivityArgs() noexcept : _rmng(nullptr), _pager(), _kmem() {
}

ActivityArgs &ActivityArgs::pager(Reference<Pager> pager) noexcept {
    _pager = pager;
    return *this;
}

ChildActivity::ChildActivity(const Reference<class Tile> &tile, const String &name,
                             const ActivityArgs &args)
    : Activity(Activity::own().alloc_sels(3), 0, tile,
               args._kmem ? args._kmem : Activity::own().kmem(), args._rmng),
      _files(),
      _mounts(),
      _exec() {
    // create pager first, to create session and obtain gate cap
    if(_tile->desc().has_virtmem()) {
        if(args._pager)
            _pager = args._pager;
        else if(Activity::own().pager())
            _pager = Activity::own().pager()->create_clone();
        // we need a pager on VM tiles
        else
            throw Exception(Errors::NOT_SUP);
    }

    if(_pager) {
        // now create activity, which implicitly obtains the gate cap from us
        const auto [eps_start, id] =
            Syscalls::create_activity(sel(), name, tile->sel(), _kmem->sel());
        _eps_start = eps_start;
        _id = id;
        // delegate activity cap to pager
        _pager->init(*this);
    }
    else {
        const auto [eps_start, id] =
            Syscalls::create_activity(sel(), name, tile->sel(), _kmem->sel());
        _eps_start = eps_start;
        _id = id;
    }
    _next_sel = Math::max(_kmem->sel() + 1, _next_sel);

    if(_resmng == nullptr) {
        capsel_t sgate_sel = alloc_sel();
        _resmng = Activity::own().resmng()->clone(*this, sgate_sel, name);
        // ensure that the child's cap space is not further ahead than ours
        // TODO improve that
        Activity::own()._next_sel = Math::max(_next_sel, Activity::own()._next_sel);
    }
    else
        delegate_obj(_resmng->sel());
}

ChildActivity::~ChildActivity() {
    try {
        stop();
    }
    catch(...) {
        // ignore
    }
}

fd_t ChildActivity::get_file(fd_t child_fd) {
    auto el = get_file_mapping(child_fd);
    if(el == _files.end())
        return FileTable::MAX_FDS;
    return el->second;
}

void ChildActivity::delegate(const KIF::CapRngDesc &crd, capsel_t dest) {
    Syscalls::exchange(sel(), crd, dest, false);
    _next_sel = Math::max(_next_sel, dest + crd.count());
}

void ChildActivity::obtain(const KIF::CapRngDesc &crd) {
    obtain(crd, Activity::own().alloc_sels(crd.count()));
}

void ChildActivity::obtain(const KIF::CapRngDesc &crd, capsel_t dest) {
    KIF::CapRngDesc own(crd.type(), dest, crd.count());
    Syscalls::exchange(sel(), own, crd.start(), true);
}

void ChildActivity::start() {
    Syscalls::activity_ctrl(sel(), KIF::Syscall::VCTRL_START, 0);
}

void ChildActivity::stop() {
    Syscalls::activity_ctrl(sel(), KIF::Syscall::VCTRL_STOP, 0);
}

int ChildActivity::wait_async(event_t event) {
    const capsel_t sels[] = {sel()};
    return Syscalls::activity_wait(sels, 1, event).first;
}

int ChildActivity::wait() {
    return wait_async(0);
}

void ChildActivity::exec(int argc, const char *const *argv, const char *const *envp) {
    do_exec(argc, argv, envp, 0);
}

}
