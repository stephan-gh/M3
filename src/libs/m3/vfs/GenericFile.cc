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
#include <base/util/Time.h>

#include <m3/com/GateStream.h>
#include <m3/session/M3FS.h>
#include <m3/vfs/FileTable.h>
#include <m3/vfs/GenericFile.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>

namespace m3 {

GenericFile::GenericFile(int flags, capsel_t caps, size_t id, epid_t mep, SendGate *sg, size_t memoff)
    : File(flags),
      _id(id),
      _sess(caps + 0, sg ? ObjCap::KEEP_CAP : 0),
      _sg(sg ? sg : new SendGate(SendGate::bind(caps + 1))),
      _mg(MemGate::bind(ObjCap::INVALID)),
      _memoff(memoff),
      _goff(),
      _off(),
      _pos(),
      _len(),
      _writing() {
    if(mep != EP_COUNT)
        _mg.ep(mep);
}

GenericFile::~GenericFile() {
    if(!(flags() & FILE_NOSESS))
        delete _sg;
}

void GenericFile::close() noexcept {
    if(_writing) {
        try {
            submit();
        }
        catch(...) {
            // ignore
        }
    }

    if(flags() & FILE_NOSESS) {
        LLOG(FS, "GenFile[" << fd() << "," << _id << "]::close()");
        try {
            send_receive_vmsg(*_sg, M3FS::CLOSE_PRIV, _id);
        }
        catch(...) {
            // ignore
        }

        VFS::free_ep(VPE::self().ep_to_sel(_mg.ep()));
    }
    else {
        if(_mg.ep() != MemGate::UNBOUND) {
            LLOG(FS, "GenFile[" << fd() << "," << _id << "]::revoke_ep(" << _mg.ep() << ")");
            capsel_t sel = VPE::self().ep_to_sel(_mg.ep());
            try {
                VPE::self().revoke(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sel), true);
            }
            catch(...) {
                // ignore
            }
            VPE::self().free_ep(_mg.ep());
        }

        // file sessions are not known to our resource manager; thus close them manually
        LLOG(FS, "GenFile[" << fd() << "]::close()");
        try {
            send_receive_vmsg(*_sg, M3FS::CLOSE);
        }
        catch(...) {
            // ignore
        }
    }
}

void GenericFile::stat(FileInfo &info) const {
    LLOG(FS, "GenFile[" << fd() << "," << _id << "]::stat()");

    GateIStream reply = send_receive_vmsg(*_sg, STAT, _id);
    receive_result(reply);
    reply >> info;
}

size_t GenericFile::seek(size_t offset, int whence) {
    LLOG(FS, "GenFile[" << fd() << "," << _id << "]::seek(" << offset << ", " << whence << ")");

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

        // first submit the written data
        if(_writing)
            submit();

        if(offset >= _goff && offset <= _goff + _len) {
            _pos = offset - _goff;
            return offset;
        }
    }
    else {
        // first submit the written data
        if(_writing)
            submit();
    }

    // now seek on the server side
    size_t off;
    GateIStream reply = !have_sess() ? send_receive_vmsg(*_sg, SEEK, _id, offset, whence)
                                     : send_receive_vmsg(*_sg, SEEK, offset, whence);
    receive_result(reply);

    reply >> _goff >> off;
    _pos = _len = 0;
    return _goff + off;
}

size_t GenericFile::read(void *buffer, size_t count) {
    delegate_ep();
    if(_writing)
        submit();

    LLOG(FS, "GenFile[" << fd() << "," << _id << "]::read("
        << count << ", pos=" << (_goff + _pos) << ")");

    if(_pos == _len) {
        Time::start(0xbbbb);
        GateIStream reply = send_receive_vmsg(*_sg, NEXT_IN, _id);
        receive_result(reply);
        Time::stop(0xbbbb);

        _goff += _len;
        reply >> _off >> _len;
        _pos = 0;
    }

    size_t amount = Math::min(count, _len - _pos);
    if(amount > 0) {
        Time::start(0xaaaa);
        if(flags() & FILE_NODATA) {
            if(count > 2)
                CPU::compute(count / 2);
        }
        else
            _mg.read(buffer, amount, _memoff + _off + _pos);
        Time::stop(0xaaaa);
        _pos += amount;
    }
    return amount;
}

size_t GenericFile::write(const void *buffer, size_t count) {
    delegate_ep();

    LLOG(FS, "GenFile[" << fd() << "," << _id << "]::write("
        << count << ", pos=" << (_goff + _pos) << ")");

    if(_pos == _len) {
        Time::start(0xbbbb);
        GateIStream reply = send_receive_vmsg(*_sg, NEXT_OUT, _id);
        receive_result(reply);
        Time::stop(0xbbbb);

        _goff += _len;
        reply >> _off >> _len;
        _pos = 0;
    }

    size_t amount = Math::min(count, _len - _pos);
    if(amount > 0) {
        Time::start(0xaaaa);
        if(flags() & FILE_NODATA) {
            if(count > 4)
                CPU::compute(count / 4);
        }
        else
            _mg.write(buffer, amount, _memoff + _off + _pos);
        Time::stop(0xaaaa);
        _pos += amount;
    }
    _writing = true;
    return amount;
}

void GenericFile::evict() {
    assert(!(flags() & FILE_NOSESS));
    assert(_mg.ep() != MemGate::UNBOUND);
    LLOG(FS, "GenFile[" << fd() << "," << _id << "]::evict()");

    // submit read/written data
    submit();

    // revoke EP cap
    capsel_t ep_sel = VPE::self().ep_to_sel(_mg.ep());
    VPE::self().revoke(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, ep_sel), true);
    _mg.ep(MemGate::UNBOUND);
}

void GenericFile::submit() {
    if(_pos > 0) {
        LLOG(FS, "GenFile[" << fd() << "," << _id << "]::submit("
            << (_writing ? "write" : "read") << ", " << _pos << ")");

        GateIStream reply = !have_sess() ? send_receive_vmsg(*_sg, COMMIT, _id, _pos)
                                         : send_receive_vmsg(*_sg, COMMIT, _pos);
        receive_result(reply);

        // if we append, the file was truncated
        _goff += _pos;
        _pos = _len = 0;
    }
    _writing = false;
}

void GenericFile::delegate_ep() {
    if(_mg.ep() == MemGate::UNBOUND) {
        assert(!(flags() & FILE_NOSESS));
        epid_t ep = VPE::self().fds()->request_ep(this);
        LLOG(FS, "GenFile[" << fd() << "," << _id << "]::delegate_ep(" << ep << ")");
        _sess.delegate_obj(VPE::self().ep_to_sel(ep));
        _mg.ep(ep);
    }
}

}
