/*
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

#include <base/log/Services.h>
#include <base/Panic.h>

#include <pci/Device.h>

#include "fifodev.h"

namespace net {

NetDriver *NetDriver::create(const char *name, m3::WorkLoop *wl, alloc_cb_func allocCallback,
                             next_buf_cb_func nextBufCallback, recv_cb_func recvCallback) {
    return new FifoDev(name, wl, allocCallback, nextBufCallback, recvCallback);
}

void FifoDev::FifoWorkItem::work() {
    char buffer[2048];
    ssize_t res = recvfrom(_dev->_in_fd, buffer, sizeof(buffer), MSG_DONTWAIT, nullptr, nullptr);
    if(res <= 0)
        return;

   SLOG(NIC, "FifoDev: received packet of " << res << " bytes");

    // read data into packet
   size_t size = static_cast<size_t>(res);
   void * pkt = 0;
   void * buf = 0;
   size_t bufSize = 0;
   if(!_dev->_allocCallback(pkt, buf, bufSize, size)) {
       SLOG(NIC, "Failed to allocate buffer to read packet.");
       return;
   }

   void * pkt_head = pkt;
   size_t readCount = 0;
   do {
       size_t readSize = std::min(bufSize, size - readCount);
       memcpy(buf, buffer + readCount, readSize);
       readCount += readSize;
       if(readCount == size)
           break;
       _dev->_nextBufCallback(pkt, buf, bufSize);
   } while(true);

   _dev->_recvCallback(pkt_head);
}

static sockaddr_un get_sock(const char *name, const char *suff) {
    sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    // we can't put that in the format string
    addr.sun_path[0] = '\0';
    snprintf(addr.sun_path + 1, sizeof(addr.sun_path) - 1, "m3_net_%s_%s", name, suff);
    return addr;
}

FifoDev::FifoDev(const char *name, m3::WorkLoop *wl, alloc_cb_func allocCallback,
                 next_buf_cb_func nextBufCallback, recv_cb_func recvCallback)
    : _in_fd(),
      _out_fd(),
      _out_sock(),
      _allocCallback(allocCallback),
      _nextBufCallback(nextBufCallback),
      _recvCallback(recvCallback),
      _linkStateChanged(true),
      _workitem(new FifoWorkItem(this)) {
    _in_fd = socket(AF_UNIX, SOCK_DGRAM, 0);
    if(_in_fd == -1)
        PANIC("Unable to create socket for " << name << "-in: " << strerror(errno));
    _out_fd = socket(AF_UNIX, SOCK_DGRAM, 0);
    if(_out_fd == -1)
        PANIC("Unable to create socket for " << name << "-out: " << strerror(errno));

    sockaddr_un in_sock = get_sock(name, "in");
    if(bind(_in_fd, (struct sockaddr*)&in_sock, sizeof(in_sock)) == -1)
        PANIC("Binding socket for " << name << "-in failed: " << strerror(errno));

    _out_sock = get_sock(name, "out");

    wl->add(_workitem, false);
}

FifoDev::~FifoDev() {
    close(_in_fd);
    close(_out_fd);
    delete _workitem;
}

void FifoDev::stop() {
    delete _workitem;
    _workitem = nullptr;
}

bool FifoDev::send(const void *packet, size_t size) {
    SLOG(NIC, "FifoDev: sending packet of " << size << " bytes");
    int res = sendto(_out_fd, packet, size, 0, (struct sockaddr*)&_out_sock, sizeof(_out_sock));
    if(res == -1)
      SLOG(NIC, "FifoDev: sending failed (" << strerror(errno) << ")");
    return res != -1;
}

m3::net::MAC FifoDev::readMAC() {
    return m3::net::MAC(0x00, 0x01, 0x02, 0x03, 0x04, 0x05);
}

bool FifoDev::linkStateChanged() {
    bool res = _linkStateChanged;
    _linkStateChanged = false;
    return res;
}

bool FifoDev::linkIsUp() {
    return true;
}

}
