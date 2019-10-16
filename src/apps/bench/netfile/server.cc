/*
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include <base/Env.h>
#include <base/util/Profile.h>

#include <m3/accel/StreamAccel.h>
#include <m3/com/Semaphore.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    NetworkManager net("net1");
    String status;

    Socket * socket = net.create(Socket::SOCK_STREAM);

    socket->blocking(true);
    socket->bind(IpAddr(192, 168, 112, 1), 1337);

    socket->listen();

    // notify client
    Semaphore::attach("net").up();

    Socket *accepted_socket = 0;
    socket->accept(accepted_socket);

    cout << "Socket accepted!\n";

    cout << "Serving...\n";
    MemGate rmem(MemGate::create_global(4096, MemGate::RW));
    fd_t rfd;
    net.as_file(accepted_socket->sd(), FILE_R, rmem, 4096, rfd);
    Reference<File> rfile = VPE::self().fds()->get(rfd);

    MemGate smem(MemGate::create_global(4096, MemGate::RW));
    fd_t sfd;
    net.as_file(accepted_socket->sd(), FILE_W, smem, 4096, sfd);
    Reference<File> sfile = VPE::self().fds()->get(sfd);

    // Creating processor
    cout << "Creating accel VPE\n";
    auto pe = PE::alloc(PEDesc(PEType::COMP_IMEM, PEISA::ACCEL_ROT13));
    std::unique_ptr<VPE> vpe(new VPE(pe, "AccelVPE"));

    std::unique_ptr<StreamAccel> accel(new StreamAccel(vpe, 1000));

    accel->connect_input(static_cast<GenericFile*>(rfile.get()));
    accel->connect_output(static_cast<GenericFile*>(sfile.get()));

    vpe->start();
    vpe->wait();

    accepted_socket->close();
    socket->close();
    delete socket;

    return 0;
}
