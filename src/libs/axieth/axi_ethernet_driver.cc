/*
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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
extern int main_example_dma_polled();
extern int main_example_dma_intr();

int main() {
    logln("Starting AXI Ethernet driver\n"_cf);

    // int error = main_example_polled();
    //  int error = main_example_intr_fifo();
    // int error = main_fifo_ping_req_example();
    //  int error = main_example_dma_polled();
    int error = main_example_dma_intr();
    if(error) {
        logln("Error: {}"_cf, error);
    }
    else {
        logln("\x1B[1;32mAll tests successful!\x1B[0;m"_cf);
    }

    // for the test infrastructure
    logln("Shutting down"_cf);
    return 0;
}
