/*
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <m3/com/GateStream.h>
#include <m3/com/OpCodes.h>
#include <m3/session/Pager.h>
#include <m3/tiles/ChildActivity.h>

namespace m3 {

Pager::Pager(capsel_t sess)
    : RefCounted(),
      ClientSession(sess, 0),
      _req_sgate(connect()),
      _child_sgate(connect().sel()),
      _pf_rgate(RecvGate::create(nextlog2<64>::val, nextlog2<64>::val)),
      _pf_sgate(connect()) {
}

Pager::Pager(capsel_t sess, capsel_t sgate)
    : RefCounted(),
      ClientSession(sess),
      _req_sgate(SendGate::bind(sgate)),
      _child_sgate(ObjCap::INVALID),
      _pf_rgate(RecvGate::bind(ObjCap::INVALID)),
      _pf_sgate(SendGate::bind(ObjCap::INVALID)) {
}

void Pager::pagefault(goff_t addr, uint access) {
    GateIStream reply = send_receive_vmsg(_req_sgate, opcodes::Pager::PAGEFAULT, addr, access);
    reply.pull_result();
}

void Pager::map_anon(goff_t *virt, size_t len, int prot, int flags) {
    GateIStream reply =
        send_receive_vmsg(_req_sgate, opcodes::Pager::MAP_ANON, *virt, len, prot, flags);
    reply.pull_result();
    reply >> *virt;
}

void Pager::map_ds(goff_t *virt, size_t len, int prot, int flags, const ClientSession &sess,
                   size_t offset) {
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << opcodes::Pager::MAP_DS << *virt << len << prot << flags << offset;
    args.bytes = os.total();

    delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sess.sel()), &args);

    ExchangeIStream is(args);
    is >> *virt;
}

void Pager::map_mem(goff_t *virt, MemGate &mem, size_t len, int prot) {
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << opcodes::Pager::MAP_MEM << *virt << len << prot;
    args.bytes = os.total();

    delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, mem.sel()), &args);

    ExchangeIStream is(args);
    is >> *virt;
}

void Pager::unmap(goff_t virt) {
    GateIStream reply = send_receive_vmsg(_req_sgate, opcodes::Pager::UNMAP, virt);
    reply.pull_result();
}

Reference<Pager> Pager::create_clone() {
    KIF::CapRngDesc caps;
    {
        KIF::ExchangeArgs args;
        ExchangeOStream os(args);
        os << opcodes::Pager::ADD_CHILD;
        args.bytes = os.total();
        caps = obtain(1, &args);
    }

    return Reference<Pager>(new Pager(caps.start()));
}

void Pager::init(ChildActivity &act) {
    // activate send and receive gate for page faults
    Syscalls::activate(act.sel() + 1, _pf_sgate.sel(), KIF::INV_SEL, 0);
    Syscalls::activate(act.sel() + 2, _pf_rgate.sel(), KIF::INV_SEL, 0);

    // delegate the session cap
    act.delegate_obj(sel());
    // delegate request send gate for child
    act.delegate_obj(_child_sgate);

    // we only need to do that for clones
    if(!(flags() & KEEP_CAP)) {
        KIF::ExchangeArgs args;
        ExchangeOStream os(args);
        os << opcodes::Pager::INIT;
        args.bytes = os.total();
        delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, act.sel()), &args);
    }
}

void Pager::clone() {
    GateIStream reply = send_receive_vmsg(_req_sgate, opcodes::Pager::CLONE);
    reply.pull_result();
}

}
