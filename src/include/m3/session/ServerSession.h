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

#pragma once

#include <base/Errors.h>
#include <base/KIF.h>

#include <m3/Syscalls.h>
#include <m3/cap/ObjCap.h>
#include <m3/server/Server.h>
#include <m3/tiles/Activity.h>

namespace m3 {

/**
 * A server session is used to represent sessions at the server side.
 */
class ServerSession : public ObjCap {
public:
    /**
     * Creates a session for the given server.
     *
     * @param crt the creator
     * @param srv_sel the server selector
     * @param sel the desired selector
     * @param auto_close send the close message if all derived session capabilities have been
     * revoked
     */
    explicit ServerSession(size_t crt, capsel_t srv_sel, capsel_t _sel = ObjCap::INVALID,
                           bool auto_close = false)
        : ObjCap(SESSION) {
        if(srv_sel != ObjCap::INVALID) {
            if(_sel == ObjCap::INVALID)
                _sel = SelSpace::get().alloc_sel();
            Syscalls::create_sess(_sel, srv_sel, crt, reinterpret_cast<word_t>(this), auto_close);
            sel(_sel);
        }
    }

    // has to be virtual, because we pass <this> as the ident to the kernel
    virtual ~ServerSession() {
    }
};

}
