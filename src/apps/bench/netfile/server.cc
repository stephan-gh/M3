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
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    NetworkManager net("net1");
    String status;

    Socket * socket = net.create(Socket::SOCK_STREAM);
    if(!socket)
        exitmsg("Socket creation failed.");

    socket->blocking(true);
    Errors::Code err = socket->bind(IpAddr(192, 168, 112, 1), 1337);
    if(err != Errors::NONE)
        exitmsg("Socket bind failed:" << Errors::to_string(err));

    socket->listen();

    Socket * accepted_socket = 0;
    err = socket->accept(accepted_socket);
    if(err != Errors::NONE)
        exitmsg("Socket accept failed:" << Errors::to_string(err));

    cout << "Socket accepted!\n";

    cout << "Serving...\n";
    MemGate rmem(MemGate::create_global(4096, MemGate::RW));
    fd_t rfd;
    err = net.as_file(accepted_socket->sd(), FILE_R, rmem, 4096, rfd);
    if(err != Errors::NONE)
        exitmsg("as_rfile failed:" << Errors::to_string(err));
    Reference<File> rfile = VPE::self().fds()->get(rfd);

    MemGate smem(MemGate::create_global(4096, MemGate::RW));
    fd_t sfd;
    err = net.as_file(accepted_socket->sd(), FILE_W, smem, 4096, sfd);
    if(err != Errors::NONE)
        exitmsg("as_sfile failed:" << Errors::to_string(err));
    Reference<File> sfile = VPE::self().fds()->get(sfd);

    // Creating processor
    cout << "Creating accel VPE\n";
    VPE *vpe = new VPE("AccelVPE", VPEArgs().pedesc(PEDesc(PEType::COMP_IMEM, PEISA::ACCEL_ROT13))
                                            .flags(VPE::MUXABLE));
    if(Errors::last != Errors::NONE)
        exitmsg("Unable to create accel VPE.\n");

    StreamAccel *accel = new StreamAccel(vpe, 1000);

    accel->connect_input(static_cast<GenericFile*>(rfile.get()));
    accel->connect_output(static_cast<GenericFile*>(sfile.get()));

    vpe->start();
    vpe->wait();

    accepted_socket->close();
    socket->close();
    delete socket;

    return 0;
}
