/**
* Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
* Economic rights: Technische Universität Dresden (Germany)
*
* This file is part of M3 (Microkernel for Minimalist Manycores).
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
#include <base/CmdArgs.h>

#include <m3/com/GateStream.h>
#include <m3/server/SimpleRequestHandler.h>
#include <m3/server/Server.h>
#include <m3/stream/Standard.h>

using namespace m3;

enum TestOp {
    TEST
};

class TestRequestHandler;
using base_class = SimpleRequestHandler<TestRequestHandler, TestOp, 1>;

class TestRequestHandler : public base_class {
public:
    explicit TestRequestHandler(WorkLoop *wl)
        : base_class(wl),
          _cnt() {
        add_operation(TEST, &TestRequestHandler::test);
    }

    void test(GateIStream &is) {
        reply_vmsg(is, _cnt++);
    }

private:
    int _cnt;
};

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-s <rgate selector>]\n";
    exit(1);
}

int main(int argc, char **argv) {
    capsel_t sels = 0;
    epid_t ep = EP_COUNT;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "s:")) != -1) {
        switch(opt) {
            case 's': {
                IStringStream is(CmdArgs::arg);
                is >> sels >> ep;
                break;
            }
            default:
                usage(argv[0]);
        }
    }

    WorkLoop wl;

    Server<TestRequestHandler> *srv;
    if(ep != EP_COUNT)
        srv = new Server<TestRequestHandler>(sels, ep, &wl, new TestRequestHandler(&wl));
    else
        srv = new Server<TestRequestHandler>("srv1", &wl, new TestRequestHandler(&wl));

    wl.run();

    delete srv;
    return 0;
}
