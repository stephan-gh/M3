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

#include <base/EnvVars.h>
#include <base/Init.h>
#include <base/Log.h>

#include <string.h>

namespace m3 {

INIT_PRIO_LOG Log Log::inst;

Log::Log() {
    const char *log = EnvVars::get("LOG");
    if(log) {
        // make a copy because strtok needs mutable access
        std::string logstr(log);
        uint64_t flags = 0;
        char *tok = strtok(logstr.data(), ",");
        while(tok) {
            if(strcmp(tok, "Info") == 0)
                flags |= LogFlags::Info;
            else if(strcmp(tok, "Debug") == 0)
                flags |= LogFlags::Debug;
            else if(strcmp(tok, "Error") == 0)
                flags |= LogFlags::Error;
            else if(strcmp(tok, "LibFS") == 0)
                flags |= LogFlags::LibFS;
            else if(strcmp(tok, "LibServ") == 0)
                flags |= LogFlags::LibServ;
            else if(strcmp(tok, "LibNet") == 0)
                flags |= LogFlags::LibNet;
            else if(strcmp(tok, "LibXlate") == 0)
                flags |= LogFlags::LibXlate;
            else if(strcmp(tok, "LibThread") == 0)
                flags |= LogFlags::LibThread;
            else if(strcmp(tok, "LibSQueue") == 0)
                flags |= LogFlags::LibSQueue;
            else if(strcmp(tok, "LibDirPipe") == 0)
                flags |= LogFlags::LibDirPipe;

            tok = strtok(NULL, ",");
        }
        inst.flags = flags;
    }
}

}
