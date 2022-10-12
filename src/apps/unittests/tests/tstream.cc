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
#include <base/stream/IStringStream.h>
#include <base/util/Math.h>

#include <m3/Test.h>
#include <m3/stream/FStream.h>

#include "../unittests.h"

using namespace m3;

static void istream() {
    int a, b;
    uint d;
    float f;

    {
        IStringStream is("1 2 0xAfd2");
        is >> a >> b >> d;
        WVASSERTEQ(a, 1);
        WVASSERTEQ(b, 2);
        WVASSERTEQ(d, 0xAfd2u);
    }

    {
        IStringStream is("  -1\t+2\n\n0XA");
        is >> a >> b >> d;
        WVASSERTEQ(a, -1);
        WVASSERTEQ(b, 2);
        WVASSERTEQ(d, 0XAu);
    }

    {
        std::string str;
        IStringStream is("  1\tabc\n\n12.4");
        is >> d >> str >> f;
        WVASSERTEQ(d, 1u);
        WVASSERTSTREQ(str.c_str(), "abc");
        WVASSERTEQ(f, 12.4f);
    }

    {
        char buf[16];
        size_t res;
        IStringStream is(" 1234 55 test\n\nfoo\n012345678901234567");
        WVASSERT(is.good());

        res = is.getline(buf, sizeof(buf));
        WVASSERTEQ(res, 13u);
        WVASSERTSTREQ(buf, " 1234 55 test");

        res = is.getline(buf, sizeof(buf));
        WVASSERTEQ(res, 0u);
        WVASSERTSTREQ(buf, "");

        res = is.getline(buf, sizeof(buf));
        WVASSERTEQ(res, 3u);
        WVASSERTSTREQ(buf, "foo");

        res = is.getline(buf, sizeof(buf));
        WVASSERTEQ(res, 15u);
        WVASSERTSTREQ(buf, "012345678901234");

        res = is.getline(buf, sizeof(buf));
        WVASSERTEQ(res, 3u);
        WVASSERTSTREQ(buf, "567");

        WVASSERT(is.eof());
    }

    struct TestItem {
        const char *str;
        float res;
    };
    struct TestItem tests[] = {
        {"1234",        1234.f   },
        {" 12.34",      12.34f   },
        {".5",          .5f      },
        {"\t +6.0e2\n", 6.0e2f   },
        {"-12.35E5",    -12.35E5f},
    };
    for(size_t i = 0; i < ARRAY_SIZE(tests); i++) {
        IStringStream is(tests[i].str);
        is >> f;
        WVASSERTEQ(f, tests[i].res);
    }
}

static void fstream() {
    int totala = 0, totalb = 0;
    float totalc = 0;
    FStream f("/mat.txt", FILE_R);
    while(!f.eof()) {
        int a, b;
        float c;
        f >> a >> b >> c;
        totala += a;
        totalb += b;
        totalc += c;
    }
    WVASSERTEQ(totala, 52184);
    WVASSERTEQ(totalb, 52184);
    // unittests with floats are really bad. the results are slightly different on x86 and Xtensa.
    // thus, we only require that the integer value is correct. this gives us at least some degree
    // of correctness here
    WVASSERTEQ(static_cast<int>(totalc), 1107);
}

void tstream() {
    RUN_TEST(istream);
    RUN_TEST(fstream);
}
