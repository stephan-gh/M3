/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

#pragma once

#include <base/Types.h>

namespace m3 {
namespace opcodes {

struct General {
    enum Operation : uint64_t {
        CONNECT = static_cast<uint64_t>(1) << 31,
    };
};

struct File {
    enum Operation {
        FSTAT,
        SEEK,
        NEXT_IN,
        NEXT_OUT,
        COMMIT,
        TRUNCATE,
        SYNC,
        CLOSE,
        CLONE_FILE,
        GET_PATH,
        GET_TMODE,
        SET_TMODE,
        SET_DEST,
        ENABLE_NOTIFY,
        REQ_NOTIFY,
    };
};

struct FileSystem {
    enum Operation {
        STAT = File::REQ_NOTIFY + 1,
        MKDIR,
        RMDIR,
        LINK,
        UNLINK,
        RENAME,
        OPEN,
        GET_MEM,
        DEL_EP,
        OPEN_PRIV,
        CLONE_META,
    };
};

struct Pipe {
    enum Operation {
        OPEN_PIPE = File::REQ_NOTIFY + 1,
        OPEN_CHAN,
        SET_MEM,
        CLOSE_PIPE,
    };
};

struct Net {
    enum Operation {
        BIND,
        LISTEN,
        CONNECT,
        ABORT,
        CREATE,
        GET_IP,
        GET_NAMESRV,
    };
};

struct ResMng {
    enum Operation {
        REG_SERV,
        UNREG_SERV,

        OPEN_SESS,
        CLOSE_SESS,

        ADD_CHILD,
        REM_CHILD,

        ALLOC_MEM,
        FREE_MEM,

        ALLOC_TILE,
        FREE_TILE,

        USE_RGATE,
        USE_SGATE,
        USE_SEM,
        USE_MOD,
    };
};

struct Pager {
    enum Operation {
        PAGEFAULT,
        INIT,
        ADD_CHILD,
        CLONE,
        MAP_ANON,
        MAP_DS,
        MAP_MEM,
        UNMAP,
        COUNT,
    };
};

struct LoadGen {
    enum Operation {
        START,
        RESPONSE,
        COUNT
    };
};

}
}
