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
#include <base/util/Time.h>

#include <m3/session/NetworkManager.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    env()->workloop()->multithreaded(4);

    NetworkManager net("net0");

    Socket *socket = net.create(Socket::SOCK_STREAM);
    if(!socket)
        exitmsg("Socket creation failed.");

    socket->blocking(true);
    Errors::Code err = socket->connect(IpAddr(192, 168, 112, 1), 1337);
    if(err != Errors::NONE)
        exitmsg("Socket connect failed:" << Errors::to_string(err));

    cout << "Socket connected!\n";
    cout << "Sending...\n";
    MemGate mem(MemGate::create_global(8192, MemGate::RW));
    fd_t fd;
    err = net.as_file(socket->sd(), FILE_RW, mem, 4096, fd);
    if(err != Errors::NONE)
        exitmsg("as_file failed:" << Errors::to_string(err));

    Reference<File> file = VPE::self().fds()->get(fd);

    cout << "Accessing socket as file: " << fd << " (" << file.get() <<")...\n";

    char buffer[1024];
    strcpy(buffer, "ABCD");
    ssize_t size = file->write(buffer, 1024);
    file->flush();

    cout << "Client Written " << size << "bytes!\n";
    cout << "Client Bytes:" << buffer << "\n";

    file->write(buffer, 1024);
    file->flush();

    file->write(buffer, 1024);
    file->flush();

    char buffer2[1024];
    while((size = file->read(buffer2, sizeof(buffer2))) >= 0) {
        cout << "Client Received " << size << "bytes!\n";
        cout << "Client Bytes: " << buffer2 << "\n";
    }

    socket->close();
    delete socket;

    return 0;
}
