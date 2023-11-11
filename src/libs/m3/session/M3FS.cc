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

#include <m3/com/GateStream.h>
#include <m3/com/OpCodes.h>
#include <m3/session/M3FS.h>
#include <m3/vfs/GenericFile.h>
#include <m3/vfs/VFS.h>

namespace m3 {

M3FS::CachedEP::~CachedEP() {
    if(ep != nullptr)
        EPMng::get().release(ep, false);
}

std::unique_ptr<GenericFile> M3FS::open(const char *path, int perms) {
    if(!(perms & FILE_NEWSESS)) {
        size_t ep_idx = get_ep();

        GateIStream reply =
            send_receive_vmsg(_gate, opcodes::FileSystem::OPEN_PRIV, path, perms, _eps[ep_idx].id);
        reply.pull_result();
        ssize_t file_id;
        reply >> file_id;

        _eps[ep_idx].file = file_id;
        return std::unique_ptr<GenericFile>(new GenericFile(
            perms, sel(), id(), static_cast<size_t>(file_id), _eps[ep_idx].ep->id(), &_gate));
    }
    else {
        KIF::ExchangeArgs args;
        ExchangeOStream os(args);
        os << opcodes::FileSystem::OPEN << perms << path;
        args.bytes = os.total();
        KIF::CapRngDesc crd = obtain(2, &args);

        return std::unique_ptr<GenericFile>(new GenericFile(perms, crd.start(), id()));
    }
}

void M3FS::close(size_t file_id) {
    for(auto &ep : _eps) {
        if(ep.file == static_cast<ssize_t>(file_id)) {
            ep.file = -1;
            break;
        }
    }

    // this is always a file without file session and therefore is closed manually
    send_receive_vmsg(_gate, opcodes::FileSystem::CLOSE_PRIV, file_id);
}

size_t M3FS::get_ep() {
    for(size_t i = 0; i < _eps.size(); ++i) {
        if(_eps[i].file == -1)
            return i;
    }

    auto ep = EPMng::get().acquire();
    size_t id = delegate_ep(ep->sel());

    _eps.push_back(CachedEP(id, ep));
    return _eps.size() - 1;
}

Errors::Code M3FS::try_stat(const char *path, FileInfo &info) noexcept {
    GateIStream reply = send_receive_vmsg(_gate, opcodes::FileSystem::STAT, path);
    Errors::Code res;
    reply >> res;
    if(res != Errors::SUCCESS)
        return res;
    reply >> info;
    return Errors::SUCCESS;
}

Errors::Code M3FS::try_mkdir(const char *path, mode_t mode) {
    GateIStream reply = send_receive_vmsg(_gate, opcodes::FileSystem::MKDIR, path, mode);
    Errors::Code res;
    reply >> res;
    return res;
}

Errors::Code M3FS::try_rmdir(const char *path) {
    GateIStream reply = send_receive_vmsg(_gate, opcodes::FileSystem::RMDIR, path);
    Errors::Code res;
    reply >> res;
    return res;
}

Errors::Code M3FS::try_link(const char *oldpath, const char *newpath) {
    GateIStream reply = send_receive_vmsg(_gate, opcodes::FileSystem::LINK, oldpath, newpath);
    Errors::Code res;
    reply >> res;
    return res;
}

Errors::Code M3FS::try_unlink(const char *path) {
    GateIStream reply = send_receive_vmsg(_gate, opcodes::FileSystem::UNLINK, path);
    Errors::Code res;
    reply >> res;
    return res;
}

Errors::Code M3FS::try_rename(const char *oldpath, const char *newpath) {
    GateIStream reply = send_receive_vmsg(_gate, opcodes::FileSystem::RENAME, oldpath, newpath);
    Errors::Code res;
    reply >> res;
    return res;
}

size_t M3FS::delegate_ep(capsel_t sel) {
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << opcodes::FileSystem::DEL_EP;
    args.bytes = os.total();

    ClientSession::delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sel, 1), &args);

    ExchangeIStream is(args);
    size_t id;
    is >> id;
    return id;
}

void M3FS::delegate(ChildActivity &act) {
    KIF::ExchangeArgs args;
    ExchangeOStream os(args);
    os << opcodes::FileSystem::CLONE_META;
    args.bytes = os.total();
    auto crd = KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sel(), 2);
    obtain_for(act, crd, &args);
}

void M3FS::serialize(Marshaller &m) {
    m << sel() << id();
}

FileSystem *M3FS::unserialize(Unmarshaller &um) {
    capsel_t sel;
    size_t id;
    um >> sel >> id;
    return new M3FS(id, sel);
}

}
