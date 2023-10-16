/*
 * riscv-rt places the global pointer in the middle of the .data section,
 * but this provides no advantages for the RoT boot layers because the .data
 * section is empty. However, linker relaxation can provide slight optimization
 * when placing the global pointer within the .rodata section (for constants).
 */
__global_pointer$ = ADDR(.rodata) + 0x800;
