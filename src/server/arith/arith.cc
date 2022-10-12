/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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

#include <base/stream/IStringStream.h>

#include <m3/com/GateStream.h>
#include <m3/server/Server.h>
#include <m3/server/SimpleRequestHandler.h>
#include <m3/stream/Standard.h>

using namespace m3;

enum ArithOp {
    CALC
};

class ArithRequestHandler;
using base_class = SimpleRequestHandler<ArithRequestHandler, ArithOp, 1>;

class ArithRequestHandler : public base_class {
public:
    explicit ArithRequestHandler(WorkLoop *wl) : base_class(wl) {
        add_operation(CALC, &ArithRequestHandler::calc);
    }

    void calc(GateIStream &is) {
        std::string str;
        is >> str;

        int a, b, res = 0;
        char op;
        IStringStream istr(str);
        istr >> a >> op >> b;
        switch(op) {
            case '+': res = a + b; break;
            case '-': res = a - b; break;
            case '*': res = a * b; break;
            case '/': res = a / b; break;
        }

        OStringStream os;
        format_to(os, "{}"_cf, res);
        reply_vmsg(is, os.str());
    }
};

int main() {
    WorkLoop wl;

    Server<ArithRequestHandler> srv("arith", &wl, std::make_unique<ArithRequestHandler>(&wl));

    wl.run();
    return 0;
}
