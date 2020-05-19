/*
 * Copyright (C) 2015-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/util/String.h>
#include <base/Errors.h>
#include <base/KIF.h>

#include <m3/com/GateStream.h>

namespace m3 {

class CapExchange {
public:
    explicit CapExchange(const KIF::Service::ExchangeData &in, KIF::Service::ExchangeData &out)
        : _in(in), _out(out), _is(in.args), _os(out.args) {
    }

    ExchangeIStream &in_args() {
        return _is;
    }
    ExchangeOStream &out_args() {
        return _os;
    }

    unsigned in_caps() const {
        return _in.caps;
    }
    void out_caps(const KIF::CapRngDesc &crd) {
        _out.caps = crd.value();
    }

private:
    const KIF::Service::ExchangeData &_in;
    KIF::Service::ExchangeData &_out;
    ExchangeIStream _is;
    ExchangeOStream _os;
};

template<class SESS>
class Handler {
public:
    typedef SESS session_type;

    virtual ~Handler() {
    }

    virtual Errors::Code open(SESS **sess, size_t crt, capsel_t, const StringRef &) = 0;
    virtual Errors::Code obtain(SESS *, size_t, CapExchange &) {
        return Errors::NOT_SUP;
    }
    virtual Errors::Code delegate(SESS *, size_t, CapExchange &) {
        return Errors::NOT_SUP;
    }
    virtual Errors::Code close(SESS *sess, size_t crt) = 0;
    virtual void shutdown() {
    }
};

}
