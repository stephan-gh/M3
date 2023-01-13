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

#include <base/Common.h>
#include <base/stream/Format.h>

#include <m3/Test.h>

#include "../unittests.h"

using namespace m3;

static void basic_arguments() {
    WVASSERTEQ(format("{}"_cf, 'a'), "a");
    WVASSERTEQ(format("{}"_cf, (char)0x30), "0");
    WVASSERTEQ(format("{}"_cf, 1234), "1234");
    WVASSERTEQ(format("{} {} {}"_cf, 1234, 7890, 3UL), "1234 7890 3");
    WVASSERTEQ(format("{0}"_cf, 1234), "1234");
    WVASSERTEQ(format("{2} {1} {0}"_cf, 1234, 7890, 3UL), "3 7890 1234");
    WVASSERTEQ(format("{} {1} {} {0} {} {}"_cf, 1234, 7890, 3UL, 10L), "1234 7890 7890 1234 3 10");
    WVASSERTEQ(format(""_cf), "");
    WVASSERTEQ(format("{{"_cf), "{");
    WVASSERTEQ(format("}}"_cf), "}");
    WVASSERTEQ(format("{{}}"_cf), "{}");
}

static void width() {
    WVASSERTEQ(format("Hello {:5}!"_cf, "x"), "Hello x    !");
    WVASSERTEQ(format("Hello {:5}!"_cf, 123U), "Hello 123  !");
    WVASSERTEQ(format("Hello {:5}!"_cf, -1), "Hello -1   !");
    WVASSERTEQ(format("Hello {:0}!"_cf, 4), "Hello 4!");
    WVASSERTEQ(format("Hello {:1}!"_cf, 4), "Hello 4!");
}

static void fill_and_align() {
    WVASSERTEQ(format("Hello {:<5}!"_cf, "x"), "Hello x    !");
    WVASSERTEQ(format("Hello {:-<5}!"_cf, "x"), "Hello x----!");
    WVASSERTEQ(format("Hello {:^5}!"_cf, "x"), "Hello   x  !");
    WVASSERTEQ(format("Hello {:>5}!"_cf, "x"), "Hello     x!");

    WVASSERTEQ(format("Hello {:<10}!"_cf, "abc"), "Hello abc       !");
    WVASSERTEQ(format("Hello {:-<10}!"_cf, "abc"), "Hello abc-------!");
    WVASSERTEQ(format("Hello {:^10}!"_cf, "abc"), "Hello    abc    !");
    WVASSERTEQ(format("Hello {:>10}!"_cf, "abc"), "Hello        abc!");

    WVASSERTEQ(format("Hello {:<10}!"_cf, -12), "Hello -12       !");
    WVASSERTEQ(format("Hello {:-<10}!"_cf, -12), "Hello -12-------!");
    WVASSERTEQ(format("Hello {:^10}!"_cf, -12), "Hello    -12    !");
    WVASSERTEQ(format("Hello {:>10}!"_cf, -12), "Hello        -12!");

    WVASSERTEQ(format("Hello {:<10}!"_cf, 1234), "Hello 1234      !");
    WVASSERTEQ(format("Hello {:-<10}!"_cf, 1234), "Hello 1234------!");
    WVASSERTEQ(format("Hello {:^10}!"_cf, 1234), "Hello    1234   !");
    WVASSERTEQ(format("Hello {:>10}!"_cf, 1234), "Hello       1234!");
}

static void numbers() {
    WVASSERTEQ(format("{:#x}"_cf, 0x1b), "0x1b");
    WVASSERTEQ(format("{:#X}"_cf, 0x1b), "0X1B");
    WVASSERTEQ(format("{:#o}"_cf, 0755), "0755");
    WVASSERTEQ(format("{:#b}"_cf, 0xff), "0b11111111");

    WVASSERTEQ(format("Hello {:+}!"_cf, 5), "Hello +5!");
    WVASSERTEQ(format("{:#x}!"_cf, 27), "0x1b!");
    WVASSERTEQ(format("Hello {:05}!"_cf, 5), "Hello 00005!");
    WVASSERTEQ(format("Hello {:05}!"_cf, -5), "Hello -0005!");
    WVASSERTEQ(format("{:#010x}!"_cf, 27), "0x0000001b!");
    WVASSERTEQ(format("{:#018x}!"_cf, -3), "0xfffffffffffffffd!");
}

static void precision() {
    WVASSERTEQ(format("Hello {:.3}!"_cf, "foobar"), "Hello foo!");
    WVASSERTEQ(format("Hello {:.0}!"_cf, "foobar"), "Hello !");
    WVASSERTEQ(format("Hello {:.10}!"_cf, "foobar"), "Hello foobar!");

    WVASSERTEQ(format("{}!"_cf, .1234f), "0.123!");
    WVASSERTEQ(format("{:.3}!"_cf, .1234f), "0.123!");
    WVASSERTEQ(format("{:.1}!"_cf, .1234f), "0.1!");
}

void tformat() {
    RUN_TEST(basic_arguments);
    RUN_TEST(width);
    RUN_TEST(fill_and_align);
    RUN_TEST(numbers);
    RUN_TEST(precision);
}
