/*
 * Copyright (C) 2016, René Küttner <rene.kuettner@tu-dresden.de>
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

#pragma once

namespace m3 {

/**
 * These flags implement the flags register for remote controlled time-multiplexing, which is used
 * to synchronize PEMux and the kernel. The kernel sets the flags register to let PEMux know
 * about the required operation. PEMux signals completion to the kernel afterwards.
 */
enum PEMuxCtrl {
    NONE                = 0,
    RESTORE             = 1 << 0, // restore operation required
    WAITING             = 1 << 1, // set by the kernel if a signal is required
    SIGNAL              = 1 << 2, // used to signal completion to the kernel
};

}
