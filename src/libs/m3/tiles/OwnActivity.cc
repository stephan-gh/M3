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

#include <m3/session/ResMng.h>
#include <m3/stream/Standard.h>
#include <m3/tiles/OwnActivity.h>
#include <m3/vfs/FileTable.h>
#include <m3/vfs/MountTable.h>
#include <m3/vfs/SerialFile.h>

namespace m3 {

INIT_PRIO_ACT OwnActivity OwnActivity::_self;

// don't revoke these. they kernel does so on exit
OwnActivity::OwnActivity()
    : Activity(KIF::SEL_ACT, KEEP_CAP, Tile::bind(KIF::SEL_TILE, TileDesc(env()->tile_desc)),
               Reference<KMem>(new KMem(KIF::SEL_KMEM)), nullptr),
      _epmng(*this),
      _ms(),
      _fds() {
    init_state();
    init_fs();

    // create stdin, stdout and stderr, if not existing
    if(!_fds->exists(STDIN_FD))
        _fds->set(STDIN_FD, _fds->alloc(std::unique_ptr<SerialFile>(new SerialFile())));
    if(!_fds->exists(STDOUT_FD))
        _fds->set(STDOUT_FD, _fds->alloc(std::unique_ptr<SerialFile>(new SerialFile())));
    if(!_fds->exists(STDERR_FD))
        _fds->set(STDERR_FD, _fds->alloc(std::unique_ptr<SerialFile>(new SerialFile())));
}

OwnActivity::~OwnActivity() {
    // ensure that we destruct these before we destruct the EP manager
    _pager.unref();
    _resmng.reset();
}

}
