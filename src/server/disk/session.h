/*
 * Copyright (C) 2018, Sebastian Reimers <sebastian.reimers@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Lukas Landgraf <llandgraf317@gmail.com>
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

#include <base/col/SList.h>

#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/session/ServerSession.h>

#define PRINT(sess, expr) SLOG(IDE, fmt((word_t)sess, "#x") << ": " << expr)

class DiskSrvSession : public m3::ServerSession {
    struct DiskSrvSGate : public m3::SListItem {
        explicit DiskSrvSGate(m3::SendGate &&_sgate) : sgate(std::move(_sgate)) {
        }
        m3::SendGate sgate;
    };

public:
    explicit DiskSrvSession(size_t dev, capsel_t srv_sel, m3::RecvGate *rgate, capsel_t _sel = m3::ObjCap::INVALID)
        : ServerSession(srv_sel, _sel), _dev(dev), _rgate(rgate), _sgates() {
    }

    size_t device() const {
        return _dev;
    }
    const m3::RecvGate &rgate() const {
        return *_rgate;
    }

    m3::Errors::Code get_sgate(m3::CapExchange &xchg) {
        if(xchg.in_caps() != 1)
            return m3::Errors::INV_ARGS;

        label_t label       = ptr_to_label(this);
        DiskSrvSGate *sgate = new DiskSrvSGate(m3::SendGate::create(
            _rgate, m3::SendGateArgs().label(label).credits(1))
        );
        _sgates.append(sgate);

        xchg.out_caps(m3::KIF::CapRngDesc(m3::KIF::CapRngDesc::OBJ, sgate->sgate.sel()));
        return m3::Errors::NONE;
    }

private:
    size_t _dev;
    m3::RecvGate *_rgate;
    m3::SList<DiskSrvSGate> _sgates;
};
