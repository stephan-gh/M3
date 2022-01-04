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

#include <base/log/Lib.h>

#include <m3/com/GateStream.h>
#include <m3/session/M3FS.h>
#include <m3/vfs/FileTable.h>
#include <m3/vfs/GenericFile.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>

namespace m3 {

GenericFile::GenericFile(int flags, capsel_t caps,
                         size_t fs_id, size_t id, epid_t mep, SendGate *sg)
    : File(flags),
      _fs_id(fs_id),
      _id(id),
      _sess(caps + 0, sg ? ObjCap::KEEP_CAP : 0),
      _sg(sg ? sg : new SendGate(SendGate::bind(caps + 1))),
      _mg(MemGate::bind(ObjCap::INVALID)),
      _goff(),
      _off(),
      _pos(),
      _len(),
      _writing() {
    if(mep != TCU::INVALID_EP)
        _mg.set_ep(new EP(EP::bind(mep)));
}

GenericFile::~GenericFile() {
    if(have_sess())
        delete _sg;
    else {
        // we never want to invalidate the EP
        delete const_cast<EP*>(_mg.ep());
        _mg.set_ep(nullptr);
    }
}

void GenericFile::close() noexcept {
    LLOG(FS, "GenFile[" << fd() << "]::evict()");

    // commit read/written data
    try {
        if(_writing)
            commit();
    }
    catch(...) {
        // ignore
    }

    if(!have_sess()) {
        auto fs = VPE::self().mounts()->get_by_id(_fs_id);
        if(fs)
            fs->close(_id);
    }
    else {
        try {
            const EP *ep = _mg.ep();
            if(ep)
                VPE::self().revoke(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, ep->sel()), true);
        }
        catch(...) {
            // ignore
        }
    }

    // file sessions are not known to our resource manager; thus close them manually
    LLOG(FS, "GenFile[" << fd() << "]::close()");
    try {
        send_receive_vmsg(*_sg, M3FS::CLOSE, _id);
    }
    catch(...) {
        // ignore
    }
}

Errors::Code GenericFile::try_stat(FileInfo &info) const {
    LLOG(FS, "GenFile[" << fd() << "]::stat()");

    GateIStream reply = send_receive_vmsg(*_sg, STAT, _id);
    Errors::Code res;
    reply >> res;
    if(res == Errors::NONE)
        reply >> info;
    return res;
}

size_t GenericFile::seek(size_t offset, int whence) {
    LLOG(FS, "GenFile[" << fd() << "]::seek(" << offset << ", " << whence << ")");

    // handle SEEK_CUR as SEEK_SET
    if(whence == M3FS_SEEK_CUR) {
        offset = _goff + _pos + offset;
        whence = M3FS_SEEK_SET;
    }

    // try to seek locally first
    if(whence == M3FS_SEEK_SET) {
        // no change?
        if(offset == _goff + _pos)
            return offset;

        // first commit the written data
        if(_writing)
            commit();

        if(offset >= _goff && offset <= _goff + _len) {
            _pos = offset - _goff;
            return offset;
        }
    }
    else {
        // first commit the written data
        if(_writing)
            commit();
    }

    // now seek on the server side
    size_t off;
    GateIStream reply = send_receive_vmsg(*_sg, SEEK, _id, offset, whence);
    reply.pull_result();

    reply >> _goff >> off;
    _pos = _len = 0;
    return _goff + off;
}

size_t GenericFile::read(void *buffer, size_t count) {
    delegate_ep();
    if(_writing)
        commit();

    LLOG(FS, "GenFile[" << fd() << "]::read(" << count << ", pos=" << (_goff + _pos) << ")");

    if(_pos == _len) {
        GateIStream reply = send_receive_vmsg(*_sg, NEXT_IN, _id);
        reply.pull_result();

        _goff += _len;
        reply >> _off >> _len;
        _pos = 0;
    }

    size_t amount = Math::min(count, _len - _pos);
    if(amount > 0) {
        if(flags() & FILE_NODATA) {
            if(count > 2)
                CPU::compute(count / 2);
        }
        else
            _mg.read(buffer, amount, _off + _pos);
        _pos += amount;
    }
    return amount;
}

size_t GenericFile::write(const void *buffer, size_t count) {
    delegate_ep();

    LLOG(FS, "GenFile[" << fd() << "]::write(" << count << ", pos=" << (_goff + _pos) << ")");

    if(_pos == _len) {
        GateIStream reply = send_receive_vmsg(*_sg, NEXT_OUT, _id);
        reply.pull_result();

        _goff += _len;
        reply >> _off >> _len;
        _pos = 0;
    }

    size_t amount = Math::min(count, _len - _pos);
    if(amount > 0) {
        if(flags() & FILE_NODATA) {
            if(count > 4)
                CPU::compute(count / 4);
        }
        else
            _mg.write(buffer, amount, _off + _pos);
        _pos += amount;
    }
    _writing = true;
    return amount;
}

void GenericFile::commit() {
    if(_pos > 0) {
        LLOG(FS, "GenFile[" << fd() << "]::commit("
            << (_writing ? "write" : "read") << ", " << _pos << ")");

        GateIStream reply = send_receive_vmsg(*_sg, COMMIT, _id, _pos);
        reply.pull_result();

        // if we append, the file was truncated
        _goff += _pos;
        _pos = _len = 0;
    }
    _writing = false;
}

void GenericFile::sync() {
    commit();

    LLOG(FS, "GenFile[" << fd() << "]::sync()");
    GateIStream reply = send_receive_vmsg(*_sg, SYNC, _id);
    reply.pull_result();
}

void GenericFile::set_tmode(TMode mode) {
    GateIStream reply = send_receive_vmsg(*_sg, Operation::SET_TMODE, _id, mode);
    reply.pull_result();
}

void GenericFile::set_signal_gate(SendGate &sg) {
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << Operation::SET_SIG;
    args.bytes = os.total();
    KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, sg.sel(), 1);
    _sess.delegate_for(VPE::self(), crd, &args);
}

void GenericFile::map(Reference<Pager> &pager, goff_t *virt, size_t fileoff, size_t len,
                      int prot, int flags) const {
    pager->map_ds(virt, len, prot, flags, _sess, fileoff);
}

void GenericFile::do_clone(VPE &vpe, KIF::CapRngDesc &crd) const {
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << Operation::CLONE;
    args.bytes = os.total();
    _sess.obtain_for(vpe, crd, &args);
}

void GenericFile::delegate_ep() {
    if(!_mg.ep()) {
        const EP &ep = _mg.acquire_ep();
        do_delegate_ep(ep);
    }
}

void GenericFile::do_delegate_ep(const EP &ep) const {
    LLOG(FS, "GenFile[" << fd() << "]::delegate_ep(" << ep.id() << ")");

    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << Operation::SET_DEST;
    args.bytes = os.total();
    KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, ep.sel(), 1);
    _sess.delegate_for(VPE::self(), crd, &args);
}

}
