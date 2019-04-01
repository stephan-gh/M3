/*
 * Copyright (C) 2018, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include "../SocketSession.h"
#include "../FileSession.h"

#include "LwipSocket.h"

using namespace m3;

LwipSocket::~LwipSocket() {
   // Revoke file session
   delete _rfile;
   if(_sfile != _rfile)
       delete _sfile;
}

ssize_t LwipSocket::send_data(m3::MemGate &mem, goff_t offset, size_t size) {
    LOG_SOCKET(this, "send_data: offset=" << offset << ", size=" << size);
    // TODO: Having mem mapped into virtual memory, would safe us a copy operation here.
    void * data = malloc(size);
    ssize_t result = -1;

    if(mem.read(data, size, offset) == Errors::NONE)
        result = send_data(data, size);
    else
        LOG_SOCKET(this, "send_data failed");

    free(data);
    return result;
}

err_t LwipSocket::errToStr(err_t err) {
    return err;
}

void LwipSocket::enqueue_data(m3::DataQueue::Item*) {
    // Nothing, can be overriden by subclass
}

Errors::Code LwipSocket::mapError(err_t err) {
   switch(err) {
       case ERR_OK: // No error, everything OK.
           return Errors::NONE;
       case ERR_MEM: // Out of memory error.
       case ERR_BUF: // Buffer error.
           return Errors::OUT_OF_MEM;
       case ERR_TIMEOUT: // Timeout.
           return Errors::TIMEOUT;
       case ERR_RTE: // Routing problem.
           return Errors::NET_UNREACHABLE;
       case ERR_INPROGRESS: // Operation in progress
           return Errors::IN_PROGRESS;
       case ERR_VAL: // Illegal value.
           return Errors::INV_ARGS;
       case ERR_WOULDBLOCK: // Operation would block.
           return Errors::WOULD_BLOCK;
       case ERR_USE: // Address in use.
           return Errors::IN_USE;
       case ERR_ALREADY: // Already connecting.
           return Errors::ALREADY_IN_PROGRESS;
       case ERR_ISCONN: // Conn already
           return Errors::IS_CONNECTED;
       case ERR_CONN: // Not connected.
           return Errors::NOT_CONNECTED;
       case ERR_IF: // Low-level netif error
           return Errors::OUT_OF_MEM;
       case ERR_ABRT: // Connection aborted.
           return Errors::CONN_ABORT;
       case ERR_RST: // Connection reset.
           return Errors::CONN_RESET;
       case ERR_CLSD: // Connection closed.
           return Errors::CONN_CLOSED;
       case ERR_ARG: // Illegal argument.
           return Errors::INV_ARGS;
       default:
           return Errors::INV_STATE;
   }
}
