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

#include <base/Panic.h>

#include <m3/com/GateStream.h>
#include <m3/com/SendGate.h>
#include <m3/Exception.h>
#include <m3/VPE.h>

namespace m3 {

class ResMng {
    explicit ResMng(capsel_t resmng, capsel_t vpe)
        : _sgate(SendGate::bind(resmng)), _vpe(vpe) {
    }

public:
    enum Operation {
        REG_SERV,
        UNREG_SERV,

        OPEN_SESS,
        CLOSE_SESS,

        ADD_CHILD,
        REM_CHILD,

        ALLOC_MEM,
        FREE_MEM,

        USE_SEM,
    };

    class ResMngException : public m3::Exception {
    public:
        explicit ResMngException(Errors::Code code, ResMng::Operation op) noexcept
            : m3::Exception(code),
              _op(op) {
        }

        ResMng::Operation operation() const {
            return _op;
        }

        const char *what() const noexcept override {
            static const char *names[] = {
                "REG_SERV",
                "UNREG_SERV",
                "OPEN_SESS",
                "CLOSE_SESS",
                "ADD_CHILD",
                "REM_CHILD",
                "ALLOC_MEM",
                "FREE_MEM",
                "USE_SEM",
            };

            OStringStream os(msg_buf, sizeof(msg_buf));
            os << "The resource manager operation " << names[_op] << " failed: "
               << Errors::to_string(code()) << " (" << code() << ")";
            return msg_buf;
        }

    private:
        ResMng::Operation _op;
    };

    explicit ResMng(capsel_t resmng) noexcept
        : _sgate(SendGate::bind(resmng)), _vpe(ObjCap::INVALID) {
    }
    ~ResMng() {
        if(_vpe != ObjCap::INVALID) {
            try {
                send_receive_vmsg(VPE::self().resmng()._sgate, REM_CHILD, _vpe);
            }
            catch(...) {
                // ignore
            }
        }
    }

    capsel_t sel() const noexcept {
        return _sgate.sel();
    }

    ResMng *clone(VPE &vpe, const String &name) {
        capsel_t sgate_sel = vpe.alloc_sel();
        clone(vpe.sel(), sgate_sel, name);
        return new ResMng(sgate_sel, vpe.sel());
    }

    void reg_service(capsel_t child, capsel_t dst, capsel_t rgate, const String &name) {
        GateIStream reply = send_receive_vmsg(_sgate, REG_SERV, child, dst, rgate, name);
        retrieve_result(REG_SERV, reply);
    }

    void unreg_service(capsel_t sel, bool notify) {
        GateIStream reply = send_receive_vmsg(_sgate, UNREG_SERV, sel, notify);
        retrieve_result(UNREG_SERV, reply);
    }

    void open_sess(capsel_t dst, const String &name) {
        GateIStream reply = send_receive_vmsg(_sgate, OPEN_SESS, dst, name);
        retrieve_result(OPEN_SESS, reply);
    }

    void close_sess(capsel_t sel) {
        GateIStream reply = send_receive_vmsg(_sgate, CLOSE_SESS, sel);
        retrieve_result(CLOSE_SESS, reply);
    }

    void alloc_mem(capsel_t sel, goff_t addr, size_t size, int perm) {
        GateIStream reply = send_receive_vmsg(_sgate, ALLOC_MEM, sel, addr, size, perm);
        retrieve_result(ALLOC_MEM, reply);
    }

    void free_mem(capsel_t sel) {
        GateIStream reply = send_receive_vmsg(_sgate, FREE_MEM, sel);
        retrieve_result(FREE_MEM, reply);
    }

    void use_sem(capsel_t sel, const char *name) {
        GateIStream reply = send_receive_vmsg(_sgate, USE_SEM, sel, name);
        retrieve_result(USE_SEM, reply);
    }

private:
    void clone(capsel_t vpe_sel, capsel_t sgate_sel, const String &name) {
        GateIStream reply = send_receive_vmsg(_sgate, ADD_CHILD, vpe_sel, sgate_sel, name);
        retrieve_result(ADD_CHILD, reply);
    }

    void retrieve_result(Operation op, GateIStream &reply) {
        Errors::Code res;
        reply >> res;
        if(res != Errors::NONE)
            throw ResMngException(res, op);
    }

    SendGate _sgate;
    capsel_t _vpe;
};

}
