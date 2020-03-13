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

#define RUN_SUITE(name)                                                 \
    m3::cout << "Running testsuite " << #name << " ...\n";              \
    name();                                                             \
    m3::cout << "\n";

#define RUN_TEST(name)                                                  \
    m3::cout << "Testing \"" << #name << "\" in " << __FILE__ << ":\n"; \
    name();                                                             \
    m3::cout << "\n";

#if defined(__host__)
void tdtu();
#endif
void tfsmeta();
void tfs();
void tbitfield();
void theap();
void tsems();
void tstream();
void tpipe();
void tstring();
void tsgate();
