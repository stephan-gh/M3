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
    "Not enough credits",
    "Not enough ringbuffer space",
    "VPE gone",
    "Pagefault",
    "No mapping",
    "Invalid endpoint",
    "Abort",
    "Reply disabled",
    "Invalid arguments",

    /* 10 */
    "Out of memory",
    "No such file or directory",
    "No permissions",
    "Not supported",
    "No free/suitable PE",
    "Invalid ELF file",
    "No space left",
    "Object does already exist",
    "Cross-filesystem link not possible",
    "Directory not empty",

    /* 20 */
    "Is a directory",
    "Is no directory",
    "Endpoint is invalid",
    "Receive buffer gone",
    "End of file",
    "Messages are waiting to be handled",
    "Reply will be sent via upcall",
    "Commit failed",
    "Out of kernel memory",

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
    "Timeout",
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
