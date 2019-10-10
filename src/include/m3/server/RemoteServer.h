/**
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of M3 (Microkernel for Minimalist Manycores).
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

#include <base/Common.h>
#include <base/stream/OStringStream.h>

#include <m3/session/ResMng.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

namespace m3 {

struct RemoteServer {
    explicit RemoteServer(VPE &vpe, const String &name)
        : srv(ObjCap::SERVICE, vpe.alloc_sels(2), ObjCap::KEEP_CAP),
          rgate(RecvGate::create_for(vpe, nextlog2<256>::val, nextlog2<256>::val)) {
        rgate.activate();

        vpe.delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, rgate.sel(), 1), srv.sel() + 1);
        VPE::self().resmng()->reg_service(vpe.sel(), srv.sel(), srv.sel() + 1, name);
    }

    void request_shutdown() {
        VPE::self().resmng()->unreg_service(srv.sel(), true);
    }

    String sel_arg() const {
        // TODO this might not work in the future
        OStringStream os;
        os << srv.sel() << " " << rgate.ep();
        return os.str();
    }

    ObjCap srv;
    RecvGate rgate;
};

}
