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

#define WVASSERTECODE(err, val)                                                            \
    ({                                                                                     \
        if((val) != -1 || errno != err) {                                                  \
            failed++;                                                                      \
            m3::cout << "! " << __FILE__ << ":" << __LINE__ << "  expected error " << #err \
                     << ", got " << val << " (errno=" << errno << ") FAILED\n";            \
        }                                                                                  \
    })

#define RUN_SUITE(name)                                    \
    m3::cout << "Running testsuite " << #name << " ...\n"; \
    name();                                                \
    m3::cout << "\n";

#define RUN_TEST(name)                                                  \
    m3::cout << "Testing \"" << #name << "\" in " << __FILE__ << ":\n"; \
    name();                                                             \
    m3::cout << "\n";

void tdir();
void tfile();
void tprocess();
void tsocket();
void ttime();
