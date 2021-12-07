/*
 * Copyright (C) 2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/time/Duration.h>

namespace m3 {

const TimeDuration TimeDuration::NANOSECOND = TimeDuration::from_nanos(1);
const TimeDuration TimeDuration::MICROSECOND = TimeDuration::from_nanos(1000);
const TimeDuration TimeDuration::MILLISECOND = TimeDuration::from_nanos(1000 * 1000);
const TimeDuration TimeDuration::SECOND = TimeDuration::from_nanos(1000 * 1000 * 1000);
const TimeDuration TimeDuration::MAX = TimeDuration::from_nanos(0xFFFFFFFFFFFFFFFF);
const TimeDuration TimeDuration::ZERO = TimeDuration::from_nanos(0);

}
