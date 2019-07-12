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

#include <base/log/Services.h>

#include <m3/server/Server.h>
#include <m3/server/EventHandler.h>
#include <m3/session/arch/host/VGA.h>
#include <m3/session/arch/host/Keyboard.h>
#include <m3/session/ServerSession.h>
#include <m3/stream/Standard.h>
#include <m3/VPE.h>

#include "Scancodes.h"
#include "VGAConsole.h"

using namespace m3;

class VGAHandler : public Handler<ServerSession> {
public:
    explicit VGAHandler(MemGate *vgamem) : _vgamem(vgamem) {
    }

    virtual Errors::Code open(ServerSession **sess, capsel_t srv_sel, const StringRef &) override {
        *sess = new ServerSession(srv_sel);
        return Errors::NONE;
    }
    virtual Errors::Code obtain(ServerSession *, KIF::Service::ExchangeData &data) override {
        if(data.caps != 1 || data.args.count != 0)
            return Errors::INV_ARGS;

        KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, _vgamem->sel());
        data.caps = crd.value();
        return Errors::NONE;
    }
    virtual Errors::Code close(ServerSession *sess) override {
        delete sess;
        return Errors::NONE;
    }

private:
    MemGate *_vgamem;
};

static Server<EventHandler<>> *kbserver;

struct ConsoleWorkItem : public WorkItem {
    void work() override {
        uint8_t sc;
        if(vgacons_check_keyb(&sc)) {
            Keyboard::Event ev;
            ev.scancode = sc;
            if(Scancodes::get_keycode(ev.isbreak, ev.keycode, ev.scancode)) {
                SLOG(KEYB, "Got " << (unsigned)ev.keycode << ":" << (unsigned)ev.isbreak);
                kbserver->handler()->broadcast(ev);
            }
        }
    }
};

int main() {
    void *vgamem = vgacons_init();

    WorkLoop wl;

    MemGate memgate = VPE::self().mem().derive(
        reinterpret_cast<uintptr_t>(vgamem), VGA::SIZE, MemGate::RW);
    Server<VGAHandler> vgasrv("vga", &wl, std::make_unique<VGAHandler>(&memgate));

    kbserver = new Server<EventHandler<>>("keyb", &wl, std::make_unique<EventHandler<>>());

    ConsoleWorkItem wi;
    wl.add(&wi, true);

    wl.run();

    delete kbserver;
    vgacons_destroy();
    return 0;
}
