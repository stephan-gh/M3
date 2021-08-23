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

#include <m3/stream/Standard.h>

#include <ctype.h>

#include "Input.h"

using namespace m3;

ssize_t Input::readline(char *buffer, size_t max) {
    size_t o = 0;

    // ensure that the line is empty
    buffer[o] = '\0';
    while(o < max) {
        // flush stdout, because cin.read blocks
        cout.flush();

        char c = cin.read();
        // EOF?
        if(c == 0x04)
            return -1;
        // TODO ^C
        if(c == 0x03)
            continue;

        switch(c) {
            // ^W
            case 0x17: {
                for(; o > 0 && isspace(buffer[o - 1]); --o)
                    cout.write_all("\b \b", 3);
                for(; o > 0 && !isspace(buffer[o - 1]); --o)
                    cout.write_all("\b \b", 3);
                cout.flush();
                break;
            }
            // backspace
            case 0x7F: {
                if(o > 0) {
                    cout.write_all("\b \b", 3);
                    cout.flush();
                    o--;
                }
                break;
            }

            default: {
                if(c == 27)
                    c = '^';

                // echo
                if(isprint(c) || c == '\n') {
                    cout.write(c);
                    cout.flush();
                }

                if(isprint(c))
                    buffer[o++] = c;
                break;
            }
        }

        if(c == '\n')
            break;
    }

    buffer[o] = '\0';
    return static_cast<ssize_t>(o);
}
