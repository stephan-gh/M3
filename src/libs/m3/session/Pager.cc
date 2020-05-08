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

#include <m3/com/GateStream.h>
#include <m3/session/Pager.h>
#include <m3/pes/VPE.h>

namespace m3 {

Pager::Pager(capsel_t sess, bool) noexcept
    : RefCounted(),
      ClientSession(sess),
      _rgate(RecvGate::create(nextlog2<64>::val, nextlog2<64>::val)),
      _own_sgate(SendGate::bind(obtain(1).start())),
      _child_sgate(SendGate::bind(obtain(1).start())),
      _close(true) {
}

Pager::Pager(capsel_t sess) noexcept
    : RefCounted(),
      ClientSession(sess),
      _rgate(RecvGate::bind(ObjCap::INVALID, nextlog2<64>::val, nextlog2<64>::val)),
      _own_sgate(SendGate::bind(obtain(1).start())),
      _child_sgate(SendGate::bind(ObjCap::INVALID)),
      _close(false) {
}

Pager::~Pager() {
    if(_close) {
        try {
            send_receive_vmsg(_own_sgate, CLOSE);
        }
        catch(...) {
            // ignore
        }
    }
}

void Pager::pagefault(goff_t addr, uint access) {
    GateIStream reply = send_receive_vmsg(_own_sgate, PAGEFAULT, addr, access);
    reply.pull_result();
}

void Pager::map_anon(goff_t *virt, size_t len, int prot, int flags) {
    GateIStream reply = send_receive_vmsg(_own_sgate, MAP_ANON, *virt, len, prot, flags);
    reply.pull_result();
    reply >> *virt;
}

void Pager::map_ds(goff_t *virt, size_t len, int prot, int flags, const ClientSession &sess,
                   size_t offset) {
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << DelOp::DATASPACE << *virt << len << prot << flags << offset;
    args.bytes = os.total();

    delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sess.sel()), &args);

    ExchangeIStream is(args);
    is >> *virt;
}

void Pager::map_mem(goff_t *virt, MemGate &mem, size_t len, int prot) {
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << DelOp::MEMGATE << *virt << len << prot;
    args.bytes = os.total();

    delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, mem.sel()), &args);

    ExchangeIStream is(args);
    is >> *virt;
}

void Pager::unmap(goff_t virt) {
    GateIStream reply = send_receive_vmsg(_own_sgate, UNMAP, virt);
    reply.pull_result();
}

Reference<Pager> Pager::create_clone() {
    KIF::CapRngDesc caps;
    {
        KIF::ExchangeArgs args;
        ExchangeOStream os(args);
        // dummy arg to distinguish from the get_sgate operation
        os << 0;
        args.bytes = os.total();
        caps = obtain(1, &args);
    }

    return Reference<Pager>(new Pager(caps.start(), true));
}

void Pager::delegate_caps(VPE &vpe) {
    // we only need to do that for clones
    if(_close) {
        // now delegate our VPE cap to the pager
        delegate_obj(vpe.sel());
    }
}

void Pager::clone() {
    GateIStream reply = send_receive_vmsg(_own_sgate, CLONE);
    reply.pull_result();
}

}
