/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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
#include <base/Env.h>
#include <base/TileDesc.h>
#include <base/stream/Serial.h>

#include <string.h>

// EXTERN_C void gem5_writefile(const char *str, uint64_t len, uint64_t offset, uint64_t file);

// EXTERN_C void puts(const char *str) {
//     size_t len = strlen(str);
//     if(m3::env()->platform == m3::Platform::GEM5) {

//         static const char *fileAddr = "stdout";
//         gem5_writefile(str, len, 0, reinterpret_cast<uint64_t>(fileAddr));
//     }
//     else {
//         static volatile uint64_t *signal    = reinterpret_cast<uint64_t*>(SERIAL_SIGNAL);

//         strcpy(reinterpret_cast<char*>(SERIAL_BUF), str);
//         *signal = len;
//         while(*signal != 0)
//             ;
//     }
// }

// EXTERN_C size_t putubuf(char *buf, ullong n, uint base) {
//     static char hexchars_small[]   = "0123456789abcdef";
//     size_t res = 0;
//     if(n >= base)
//         res += putubuf(buf, n / base, base);
//     buf[res] = hexchars_small[n % base];
//     return res + 1;
// }

// EXTERN_C void putu(ullong n, uint base) {
//     char buf[32];
//     size_t len = putubuf(buf, n, base);
//     buf[len] = 0;
//     puts(buf);
// }

// for __verbose_terminate_handler from libsupc++
EXTERN_C ssize_t write(int, const void *, size_t) {
    return 0;
}
EXTERN_C int sprintf(char *, const char *, ...) {
    return 0;
}

void *stderr;
EXTERN_C int fputs(const char *str, void *) {
    m3::Serial::get() << str;
    return 0;
}
EXTERN_C int fputc(int c, void *) {
    m3::Serial::get().write(c);
    return -1;
}
EXTERN_C size_t fwrite(const void *str, UNUSED size_t size, size_t nmemb, void *) {
    // assert(size == 1);
    const char *s = reinterpret_cast<const char *>(str);
    auto &ser = m3::Serial::get();
    while(nmemb-- > 0)
        ser.write(*s++);
    return 0;
}

class StandaloneEnvBackend : public m3::Gem5EnvBackend {
public:
    explicit StandaloneEnvBackend() {
    }

    virtual void init() override {
        m3::Serial::init("standalone", m3::env()->tile_id);
    }

    virtual bool extend_heap(size_t) override {
        return false;
    }

    virtual void exit(int) override {
        m3::Machine::shutdown();
    }
};

extern void *_bss_end;

void m3::Env::init() {
    env()->set_backend(new StandaloneEnvBackend());
    env()->backend()->init();
    env()->call_constr();
}
