#!/bin/awk -f

match($0, /\[\s*[0-9]+\] ([a-zA-Z_0-9]+) \(.*\) ([0-9]+) ([0-9]+)/, m) {
    if(last_syscall) {
        printf("%15s: %d\n", "<app>", m[2] - last_syscall)
        app_time = app_time + (m[2] - last_syscall)
    }
    printf("%15s: %d\n", m[1], m[3] - m[2])
    sys_time = sys_time + (m[3] - m[2])
    last_syscall = m[3]
}

END {
    print("App time:", app_time, " cycles")
    print("Sys time:", sys_time, " cycles")
}
