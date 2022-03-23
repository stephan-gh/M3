/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

#include <m3/net/TcpSocket.h>
#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main(int argc, char **argv) {
    if(argc != 3) {
        cerr << "Usage: " << argv[0] << " <ip> <port>\n";
        return 1;
    }

    IpAddr dest = IStringStream::read_from<IpAddr>(argv[1]);
    int port = IStringStream::read_from<port_t>(argv[2]);

    NetworkManager net("net");

    auto socket = TcpSocket::create(net);

    socket->connect(Endpoint(dest, port));

    socket->blocking(false);

    while(!cin.eof()) {
        try {
            String line;
            cin >> line;
            socket->send(line.c_str(), line.length());
        }
        catch(const Exception &e) {
            cerr << e.what() << "\n";
            return 1;
        }
    }
    return 0;
}
