#include <string.h>

#include "platform.h"

extern "C" int main();
extern "C" void gem5_shutdown(uint64_t delay);
extern "C" void gem5_writefile(const char *str, uint64_t len, uint64_t offset, uint64_t file);

namespace m3 {
class Env {
    __attribute__((used)) void exit(int, bool) {
        gem5_shutdown(0);
    }
};
}

extern "C" int puts(const char *str) {
    static const char *fileAddr = "stdout";
    gem5_writefile(str, strlen(str), 0, reinterpret_cast<uint64_t>(fileAddr));
    return 0;
}

extern "C" void exit(int) {
    gem5_shutdown(0);
    UNREACHED;
}

extern "C" void env_run() {
    exit(main());
}

void init() {
}

void deinit() {
}
