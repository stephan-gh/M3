#include <base/Common.h>
#include <base/mem/Heap.h>
#include <base/stream/Serial.h>
#include <base/Env.h>
#include <base/PEDesc.h>
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

class StandaloneEnvBackend : public m3::Gem5EnvBackend {
public:
    explicit StandaloneEnvBackend() {
    }

    virtual void init() override {
        m3::Serial::init("standalone", m3::env()->pe_id);
    }

    virtual void reinit() override {
        // not used
    }

    virtual bool extend_heap(size_t) override {
        return false;
    }

    virtual void exit(int) override {
        m3::Machine::shutdown();
    }
};

extern void *_bss_end;

EXTERN_C void init_env(m3::Env *e) {
    m3::Heap::init();
    e->backend_addr = reinterpret_cast<uint64_t>(new StandaloneEnvBackend());
}