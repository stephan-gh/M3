/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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
#include <base/ELF.h>

#include <m3/stream/FStream.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>

using namespace m3;

static const char *phtypes[] = {"NULL   ", "LOAD   ", "DYNAMIC", "INTERP ",
                                "NOTE   ", "SHLIB  ", "PHDR   ", "TLS    "};

template<typename ELF_EH, typename ELF_PH>
static void parse(FStream &bin) {
    bin.seek(0, M3FS_SEEK_SET);

    ELF_EH header;
    if(bin.read(&header, sizeof(header)).unwrap() != sizeof(header))
        exitmsg("Invalid ELF-file: unable to read arch-specific ELF header"_cf);

    println("Program Headers:"_cf);
    println("  Type    Offset   VirtAddr   PhysAddr   FileSiz    MemSiz     Flg Align"_cf);

    size_t off = header.e_phoff;
    for(uint i = 0; i < header.e_phnum; ++i, off += header.e_phentsize) {
        ELF_PH pheader;
        auto noff = bin.seek(off, M3FS_SEEK_SET);
        if(noff != off)
            exitmsg("Seek to program header failed: expected {}, got {}"_cf, off, noff);

        size_t pheader_size = bin.read(&pheader, sizeof(pheader)).unwrap();
        if(pheader_size != sizeof(pheader))
            exitmsg("Reading program header failed: read only {} bytes"_cf, pheader_size);

        println("  {} {:#08x} {:#010x} {:#010x} {:#010x} {:#010x} {}{}{} {:#x}"_cf,
                pheader.p_type < ARRAY_SIZE(phtypes) ? phtypes[pheader.p_type] : "???????",
                pheader.p_offset, pheader.p_vaddr, pheader.p_paddr, pheader.p_filesz,
                pheader.p_memsz, (pheader.p_flags & PF_R) ? "R" : " ",
                (pheader.p_flags & PF_W) ? "W" : " ", (pheader.p_flags & PF_X) ? "E" : " ",
                pheader.p_align);
    }
}

int main(int argc, char **argv) {
    if(argc < 2)
        exitmsg("Usage: {} <bin>"_cf, argv[0]);

    FStream bin(argv[1], FILE_R);

    /* load and check ELF header */
    ElfEh header;
    if(bin.read(&header, sizeof(header)).unwrap() != sizeof(header))
        exitmsg("Invalid ELF-file: unable to read ELF header"_cf);

    if(header.e_ident[0] != '\x7F' || header.e_ident[1] != 'E' || header.e_ident[2] != 'L' ||
       header.e_ident[3] != 'F')
        exitmsg("Invalid ELF-file"_cf);

    if(header.e_ident[EI_CLASS] == ELFCLASS32)
        parse<Elf32_Ehdr, Elf32_Phdr>(bin);
    else
        parse<Elf64_Ehdr, Elf64_Phdr>(bin);
    return 0;
}
