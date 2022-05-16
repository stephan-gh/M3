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

use crate::net::Sd;

#[repr(usize)]
pub enum NetLogEvent {
    SubmitData = 1,
    SentPacket,
    RecvPacket,
    FetchData,
    RecvConnected,
    RecvClosed,
    RecvRemoteClosed,
    StartedWaiting,
    StoppedWaiting,
}

#[cfg(target_vendor = "gem5")]
#[inline(always)]
pub fn log_net(ev: NetLogEvent, sd: Sd, arg: usize) {
    let msg = ev as u64 | (sd as u64) << 8 | (arg as u64) << 16;
    base::cpu::gem5_debug(msg);
}

#[cfg(not(target_vendor = "gem5"))]
pub fn log_net(_ev: NetLogEvent, _sd: Sd, _arg: usize) {
}
