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

#pragma once

#include <base/Common.h>

#include <m3/com/SendGate.h>
#include <m3/com/MemGate.h>
#include <m3/session/ClientSession.h>
#include <m3/vfs/File.h>
#include <m3/Exception.h>
#include <m3/pes/VPE.h>

namespace m3 {

class M3FS;

class GenericFile : public File {
    friend class FileTable;

public:
    enum Operation {
        STAT,
        SEEK,
        NEXT_IN,
        NEXT_OUT,
        COMMIT,
        SYNC,
        CLOSE,
        SET_TMODE,
        COUNT,
    };

    explicit GenericFile(int flags, capsel_t caps);

    /**
     * @return true if there is still data to read or write without contacting the server
     */
    bool has_data() const noexcept {
        return _pos < _len;
    }

    virtual Errors::Code try_stat(FileInfo &info) const override;

    virtual size_t seek(size_t offset, int whence) override;

    virtual size_t read(void *buffer, size_t count) override;
    virtual size_t write(const void *buffer, size_t count) override;

    virtual void flush() override {
        if(_writing)
            commit();
    }

    virtual void sync() override;

    virtual void map(Reference<Pager> &pager, goff_t *virt, size_t fileoff, size_t len,
                     int prot, int flags) const override;

    virtual void set_tmode(TMode mode) override;

    virtual char type() const noexcept override {
        return 'F';
    }

    void connect(EP &sep, EP &mep) const {
        _sg.activate_on(sep);
        _sess.delegate_obj(mep.sel());
    }

    virtual Reference<File> clone() const override {
        KIF::CapRngDesc crd = _sess.obtain(2);
        return Reference<File>(new GenericFile(flags(), crd.start()));
    }

    virtual void delegate(VPE &vpe) override {
        KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, _sess.sel(), 2);
        _sess.obtain_for(vpe, crd);
    }

    virtual void serialize(Marshaller &m) override {
        m << flags() << _sess.sel();
    }

    static File *unserialize(Unmarshaller &um) {
        int fl;
        capsel_t caps;
        um >> fl >> caps;
        return new GenericFile(fl, caps);
    }

private:
    virtual void close() noexcept override;

    void commit();
    void delegate_ep();

    mutable ClientSession _sess;
    mutable SendGate _sg;
    MemGate _mg;
    size_t _memoff;
    size_t _goff;
    size_t _off;
    size_t _pos;
    size_t _len;
    bool _writing;
};

}
