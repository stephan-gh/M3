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

#include <m3/com/Semaphore.h>
#include <m3/stream/Standard.h>

#include <endian.h>

#include "handler.h"
#include "m3/com/GateStream.h"

using namespace m3;

TCUOpHandler::TCUOpHandler()
    : _rgate(RecvGate::create_named("req")),
      _result(MemGate::create_global(MAX_RESULT_SIZE, MemGate::W)),
      _last_req() {
}

OpHandler::Result TCUOpHandler::receive(Package &pkg) {
    _last_req = new GateIStream(receive_msg(_rgate));

    // There is an edge case where the package size is 6, If thats the case, check if we got the
    // end flag from the client. In that case its time to stop the benchmark.
    if(memcmp(_last_req->message().data, "ENDNOW", 6) == 0) {
        reply_vmsg(*_last_req, 0);
        delete _last_req;
        return Result::STOP;
    }

    UNUSED auto res = from_bytes(_last_req->message().data, _last_req->message().length, pkg);
    assert(res != Result::INCOMPLETE);

    return Result::READY;
}

bool TCUOpHandler::respond(size_t bytes) {
    char buffer[1024];
    memset(buffer, 0, sizeof(buffer));

    size_t total = 0;
    while(total < bytes) {
        size_t amount = Math::min(total - bytes, sizeof(buffer));
        _result.write(buffer, amount, total);
        total += amount;
    }

    reply_vmsg(*_last_req, bytes);
    delete _last_req;

    return true;
}

Option<size_t> TCUOpHandler::send(const void *, size_t) {
    // unused
    return None;
}
