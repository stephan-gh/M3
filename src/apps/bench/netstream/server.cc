/*
 * Copyright (C) 2019, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    env()->workloop()->multithreaded(4);

    NetworkManager net("net1");
    String status;

    Socket *socket = net.create(Socket::SOCK_STREAM);
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
    accepted_socket->blocking(true);

    char request[1024];
    while(true) {
        ssize_t len = accepted_socket->recv(request, sizeof(request));
        if(len <= 0) {
            if(len == -Errors::INV_STATE)
                exitmsg("Client disconnected.");
            else
                exitmsg("Received invalid data: " << len);
        }

        while(accepted_socket->send(request, static_cast<size_t>(len)) <= 0) {
        }
    }

    delete accepted_socket;
    delete socket;
}
