#include "../common.h"
#include "platform.h"

//testcase specific defines
#define TESTCASE_FAILED      0xAFFEAFFE
#define TESTCASE_PASSED      0x11111111

#define TESTCASE_RESULT_ADDR 0x10071000

volatile uint64_t *ui64_ptr = (uint64_t*)TESTCASE_RESULT_ADDR;
volatile uint32_t *ui32_ptr = (uint32_t*)TESTCASE_RESULT_ADDR;

extern "C" int main();

extern "C" void env_run() {
    main();
}

void init() {
    ui64_ptr[0] = TESTCASE_FAILED;
    ui64_ptr[1] = 0;
}

void deinit() {
    ui64_ptr[0] = TESTCASE_PASSED;
}
