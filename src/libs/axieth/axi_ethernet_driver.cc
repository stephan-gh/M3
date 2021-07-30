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

#include <base/stream/Serial.h>

using namespace m3;

extern int main_example_polled();
extern int main_example_intr_fifo();
extern int main_fifo_ping_req_example();


int main() {
    Serial::get() << "Starting AXI Ethernet driver\n\n";

    //int error = main_example_polled();
    // int error = main_example_intr_fifo();
    int error = main_fifo_ping_req_example();
    if (error){
        Serial::get() << "Error: " << error << "\n";
    } else {
        Serial::get() << "\x1B[1;32mAll tests successful!\x1B[0;m\n";
    }

    // for the test infrastructure
    Serial::get() << "Shutting down\n";
    return 0;
}
