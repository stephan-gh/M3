def build(gen, env):
    env.m3_exe(
        gen,
        out = 'fstrace-m3fs',
        ins = ['player_main.cc', 'traceplayer.cc', 'buffer.cc', 'traces.cc'] + env.glob('traces/*.c')
    )

    hostenv = env.hostenv.clone()
    hostenv['CPPFLAGS'] += ['-D__LINUX__=1']
    bin = hostenv.cxx_exe(
        gen,
        out = 'strace2cpp',
        ins = ['strace2cpp.cc', 'tracerecorder.cc', 'opdescr.cc']
    )
    hostenv.install(gen, hostenv['TOOLDIR'], bin)
