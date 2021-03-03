/*
 * Copyright (C) 2016-2017, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/Errors.h>

namespace m3 {

static const char *errmsgs[] = {
    /* 0 */
    "No error",
    "No memory endpoint",
    "No send endpoint",
    "No receive endpoint",
    "Foreign endpoint",
    "SEND/REPLY with wrong endpoint",
    "Receiver gone",
    "Receive buffer misaligned",
    "Receive buffer full",
    "Replies disabled",
    /* 10 */
    "Out of bounds",
    "No credits",
    "No permission",
    "Invalid message offset",
    "Pagefault",
    "Command aborted",
    "Unknown TCU command",
    "Message too large for receive buffer",
    "Invalid reply EPs in receive EP",
    "Invalid credit EP in send EP",
    /* 20 */
    "Invalid msg_sz in send EP",
    "Receiver is busy, retry command",
    "Timeout while waiting for memory response",
    "Timeout while waiting for NoC response",
    "Data contains page boundary",
    "Message is not 16-byte aligned",
    "Invalid arguments",
    "VPE gone",
    "Out of memory",
    "No such file or directory",
    /* 30 */
    "Not supported",
    "No free/suitable PE",
    "Invalid ELF file",
    "No space left",
    "Object does already exist",
    "Cross-filesystem link not possible",
    "Directory not empty",
    "Is a directory",
    "Is no directory",
    "Endpoint is invalid",
    /* 40 */
    "End of file",
    "Messages are waiting to be handled",
    "Reply will be sent via upcall",
    "Commit failed",
    "Out of kernel memory",
    "Not found",
    "Not revocable",
    "Timeout",

    /* Socket */
    "In use",
    "Invalid state",
    "Would block",
    "In progress",
    "Already in progress",
    "Socket is not connected",
    "Socket is connected",
    "Connection aborted",
    "Connection reset/refused by peer",
    "Connection closed gracefully",
    "Network is unreachable",
    "Socket closed"
};

const char *Errors::to_string(Code code) {
    size_t idx = code;
    if(idx < ARRAY_SIZE(errmsgs))
        return errmsgs[idx];
    return "Unknown error";
}

}
