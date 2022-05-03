/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

#include <base/Common.h>
#include <base/time/Duration.h>

#include <m3/vfs/File.h>
#include <m3/vfs/FileRef.h>

enum Mode {
    INDIR = 0,
    DIR = 1,
    DIR_SIMPLE = 2,
    DIR_MULTI = 3,
};

void chain_direct(m3::FileRef<m3::GenericFile> &in, m3::FileRef<m3::GenericFile> &out, size_t num,
                  m3::CycleDuration comptime, Mode mode);
void chain_direct_multi(m3::FileRef<m3::GenericFile> &in, m3::FileRef<m3::GenericFile> &out,
                        size_t num, m3::CycleDuration comptime, Mode mode);
void chain_indirect(m3::FileRef<m3::GenericFile> &in, m3::FileRef<m3::GenericFile> &out, size_t num,
                    m3::CycleDuration comptime);
