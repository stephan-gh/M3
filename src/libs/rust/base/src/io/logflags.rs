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

use bitflags::bitflags;

use core::str;

bitflags! {
    /// All log flags used in M³
    ///
    /// Logging in M³ is controlled at runtime via the environment variable `LOG`. Additionally, it
    /// can be passed to all components when starting M³ via `M3_LOG`. Any component can then use
    /// the `log` macro to log something. The available flags are kept here.
    ///
    /// There are three general flags: `Info`, `Debug`, and `Error`. These are used by various
    /// components and Info and Error is enabled by default. These flags are also used in some
    /// applications, so that we don't need to add new flags for applications.
    ///
    /// Additionally, there are per-component flags such as `KernEPs`, `ResMngChild`, or `PgReqs`
    /// that control the logging of certain aspects within a specific component.
    ///
    /// Note however that the log flags are hard coded to `Info` and `Error` in bench mode
    /// (`M3_BUILD=bench`)!
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct LogFlags : u128 {
        /// General: informational output (enabled by default)
        const Info          = 1 << 0;
        /// General: debugging output (disable by default)
        const Debug         = 1 << 1;
        /// General: error output (enabled by default)
        const Error         = 1 << 2;

        #[doc(hidden)]
        const __lib_start = 3;

        /// libraries: file system operations
        const LibFS         = 1 << (Self::__lib_start.bits() + 0);
        /// libraries: server operations
        const LibServ       = 1 << (Self::__lib_start.bits() + 1);
        /// libraries: requests to servers
        const LibServReqs   = 1 << (Self::__lib_start.bits() + 2);
        /// libraries: networking events
        const LibNet        = 1 << (Self::__lib_start.bits() + 3);
        /// libraries: global<->phys address translations
        const LibXlate      = 1 << (Self::__lib_start.bits() + 4);
        /// libraries: thread switching
        const LibThread     = 1 << (Self::__lib_start.bits() + 5);
        /// libraries: send queue
        const LibSQueue     = 1 << (Self::__lib_start.bits() + 6);
        /// libraries: direct pipe
        const LibDirPipe    = 1 << (Self::__lib_start.bits() + 7);

        #[doc(hidden)]
        const __kern_start = Self::__lib_start.bits() + 8;

        /// Kernel: endpoint configurations for user tiles
        const KernEPs       = 1 << (Self::__kern_start.bits() + 0);
        /// Kernel: endpoint configurations for the kernel tile
        const KernKEPs      = 1 << (Self::__kern_start.bits() + 1);
        /// Kernel: system calls
        const KernSysc      = 1 << (Self::__kern_start.bits() + 2);
        /// Kernel: capability operations
        const KernCaps      = 1 << (Self::__kern_start.bits() + 3);
        /// Kernel: memory allocations/frees
        const KernMem       = 1 << (Self::__kern_start.bits() + 4);
        /// Kernel: kernel memory objects
        const KernKMem      = 1 << (Self::__kern_start.bits() + 5);
        /// Kernel: service calls
        const KernServ      = 1 << (Self::__kern_start.bits() + 6);
        /// Kernel: sendqueue operations
        const KernSQueue    = 1 << (Self::__kern_start.bits() + 7);
        /// Kernel: activities
        const KernActs      = 1 << (Self::__kern_start.bits() + 8);
        /// Kernel: TileMux calls
        const KernTMC       = 1 << (Self::__kern_start.bits() + 9);
        /// Kernel: tile operations
        const KernTiles     = 1 << (Self::__kern_start.bits() + 10);
        /// Kernel: sent upcalls
        const KernUpcalls   = 1 << (Self::__kern_start.bits() + 11);
        /// Kernel: slab allocations/frees
        const KernSlab      = 1 << (Self::__kern_start.bits() + 12);
        /// Kernel: TCU operations
        const KernTCU       = 1 << (Self::__kern_start.bits() + 13);

        #[doc(hidden)]
        const __mux_start = Self::__kern_start.bits() + 14;

        /// TileMux: basic activity operations
        const MuxActs       = 1 << (Self::__mux_start.bits() + 0);
        /// TileMux: TileMux calls
        const MuxCalls      = 1 << (Self::__mux_start.bits() + 1);
        /// TileMux: context switches
        const MuxCtxSws     = 1 << (Self::__mux_start.bits() + 2);
        /// TileMux: sidecalls (TileMux <-> Kernel)
        const MuxSideCalls  = 1 << (Self::__mux_start.bits() + 3);
        /// TileMux: foreign messages
        const MuxForMsgs    = 1 << (Self::__mux_start.bits() + 4);
        /// TileMux: CU requests
        const MuxCUReqs     = 1 << (Self::__mux_start.bits() + 5);
        /// TileMux: page table allocations/frees
        const MuxPTs        = 1 << (Self::__mux_start.bits() + 6);
        /// TileMux: timer IRQs
        const MuxTimer      = 1 << (Self::__mux_start.bits() + 7);
        /// TileMux: interrupts
        const MuxIRQs       = 1 << (Self::__mux_start.bits() + 8);
        /// TileMux: sendqueue operations
        const MuxSQueue     = 1 << (Self::__mux_start.bits() + 9);
        /// TileMux: quota operations
        const MuxQuotas     = 1 << (Self::__mux_start.bits() + 10);

        #[doc(hidden)]
        const __resmng_start = Self::__mux_start.bits() + 11;

        /// Resource manager (root/pager): child operations
        const ResMngChild   = 1 << (Self::__resmng_start.bits() + 0);
        /// Resource manager (root/pager): gate operations
        const ResMngGate    = 1 << (Self::__resmng_start.bits() + 1);
        /// Resource manager (root/pager): semaphore operations
        const ResMngSem     = 1 << (Self::__resmng_start.bits() + 2);
        /// Resource manager (root/pager): service operations
        const ResMngServ    = 1 << (Self::__resmng_start.bits() + 3);
        /// Resource manager (root/pager): sendqueue operations
        const ResMngSQueue  = 1 << (Self::__resmng_start.bits() + 4);
        /// Resource manager (root/pager): memory operations
        const ResMngMem     = 1 << (Self::__resmng_start.bits() + 5);
        /// Resource manager (root/pager): tile operations
        const ResMngTiles   = 1 << (Self::__resmng_start.bits() + 6);
        /// Resource manager (root/pager): serial operations
        const ResMngSerial  = 1 << (Self::__resmng_start.bits() + 7);

        #[doc(hidden)]
        const __net_start = Self::__resmng_start.bits() + 8;

        /// Net: session operations
        const NetSess       = 1 << (Self::__net_start.bits() + 0);
        /// Net: data transfers
        const NetData       = 1 << (Self::__net_start.bits() + 1);
        /// Net: allocated/freed ports
        const NetPorts      = 1 << (Self::__net_start.bits() + 2);
        /// Net: NIC operations
        const NetNIC        = 1 << (Self::__net_start.bits() + 3);
        /// Net: NIC checksum failures
        const NetNICChksum  = 1 << (Self::__net_start.bits() + 4);
        /// Net: more verbose NIC operations
        const NetNICDbg     = 1 << (Self::__net_start.bits() + 5);
        /// Net: smoltcp prints
        const NetSmolTCP    = 1 << (Self::__net_start.bits() + 6);
        /// Net: polling / sleeping
        const NetPoll       = 1 << (Self::__net_start.bits() + 7);

        #[doc(hidden)]
        const __fs_start = Self::__net_start.bits() + 8;

        /// m3fs: general information (superblock, ...)
        const FSInfo        = 1 << (Self::__fs_start.bits() + 0);
        /// m3fs: session operations
        const FSSess        = 1 << (Self::__fs_start.bits() + 1);
        /// m3fs: inode/block bitmap allocations
        const FSAlloc       = 1 << (Self::__fs_start.bits() + 2);
        /// m3fs: file/meta buffer
        const FSBuf         = 1 << (Self::__fs_start.bits() + 3);
        /// m3fs: directory operations
        const FSDirs        = 1 << (Self::__fs_start.bits() + 4);
        /// m3fs: inode operations
        const FSINodes      = 1 << (Self::__fs_start.bits() + 5);
        /// m3fs: link creation/removal
        const FSLinks       = 1 << (Self::__fs_start.bits() + 6);
        /// m3fs: directory traversal
        const FSFind        = 1 << (Self::__fs_start.bits() + 7);

        #[doc(hidden)]
        const __pg_start = Self::__fs_start.bits() + 8;

        /// Paging: mapping operations
        const PgMap         = 1 << (Self::__pg_start.bits() + 0);
        /// Paging: individual pages of mapping operations
        const PgMapPages    = 1 << (Self::__pg_start.bits() + 1);
        /// Paging: requests to the pager
        const PgReqs        = 1 << (Self::__pg_start.bits() + 2);
        /// Paging: memory allocations
        const PgMem         = 1 << (Self::__pg_start.bits() + 3);

        #[doc(hidden)]
        const __vt_start = Self::__pg_start.bits() + 4;

        /// vterm: requests
        const VTReqs        = 1 << (Self::__vt_start.bits() + 0);
        /// vterm: input/output operations
        const VTInOut       = 1 << (Self::__vt_start.bits() + 1);
        /// vterm: sent events
        const VTEvents      = 1 << (Self::__vt_start.bits() + 2);

        #[doc(hidden)]
        const __hmux_start = Self::__vt_start.bits() + 3;

        /// hashmux: requests
        const HMuxReqs      = 1 << (Self::__hmux_start.bits() + 0);
        /// hashmux: input/output operations
        const HMuxInOut     = 1 << (Self::__hmux_start.bits() + 1);
        /// hashmux: more verbose output
        const HMuxDbg       = 1 << (Self::__hmux_start.bits() + 2);

        #[doc(hidden)]
        const __disk_start = Self::__hmux_start.bits() + 3;

        /// disk: requests
        const DiskReqs      = 1 << (Self::__disk_start.bits() + 0);
        /// disk: channel operations
        const DiskChan      = 1 << (Self::__disk_start.bits() + 1);
        /// disk: device operations
        const DiskDev       = 1 << (Self::__disk_start.bits() + 2);
        /// disk: controller operations
        const DiskCtrl      = 1 << (Self::__disk_start.bits() + 3);
        /// disk: more verbose output
        const DiskDbg       = 1 << (Self::__disk_start.bits() + 4);

        #[doc(hidden)]
        const __pipe_start = Self::__disk_start.bits() + 5;

        /// pipe: requests
        const PipeReqs      = 1 << (Self::__pipe_start.bits() + 0);
        /// pipe: data transfers / state changes
        const PipeData      = 1 << (Self::__pipe_start.bits() + 1);
    }
}

impl str::FromStr for LogFlags {
    type Err = bitflags::parser::ParseError;

    fn from_str(flags: &str) -> Result<Self, Self::Err> {
        Ok(Self(flags.parse()?))
    }
}
