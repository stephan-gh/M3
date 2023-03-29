def build(gen, env):
    standalone = [
        'empty', 'find', 'leveldb', 'nginx', 'sha256sum', 'sort', 'sqlite', 'tar', 'untar'
    ]
    pipe = [
        'cat_awk_awk', 'cat_awk_cat',
        'cat_wc_cat', 'cat_wc_wc',
        'grep_awk_awk', 'grep_awk_grep',
        'grep_wc_grep', 'grep_wc_wc',
    ]
    standalone_files = ['traces/' + f + '.c' for f in standalone]
    pipe_files = ['traces/' + f + '.c' for f in pipe]

    base_obj = []
    for f in ['player_main.cc', 'traceplayer.cc', 'buffer.cc']:
        base_obj += [env.cxx(gen, out=f + '.o', ins=[f])]

    standalone_env = env.clone()
    standalone_env['CPPFLAGS'] += ['-DM3_TRACE_STANDALONE=1']
    standalone_traces = standalone_env.cxx(gen, out='traces-standalone.o', ins=['traces.cc'])

    standalone_env.m3_exe(
        gen,
        out='fstrace-m3fs',
        ins=base_obj + [standalone_traces] + standalone_files
    )

    pipe_env = env.clone()
    pipe_env['CPPFLAGS'] += ['-DM3_TRACE_PIPE=1']
    pipe_traces = pipe_env.cxx(gen, out='traces-pipe.o', ins=['traces.cc'])
    pipe_env.m3_exe(
        gen,
        out='fstrace-m3fs-pipe',
        ins=base_obj + [pipe_traces] + pipe_files
    )

    hostenv = env.hostenv.clone()
    hostenv['CPPFLAGS'] += ['-D__LINUX__=1']
    bin = hostenv.cxx_exe(
        gen,
        out='strace2cpp',
        ins=['strace2cpp.cc', 'tracerecorder.cc', 'opdescr.cc']
    )
    hostenv.install(gen, hostenv['TOOLDIR'], bin)
