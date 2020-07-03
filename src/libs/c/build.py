def build(gen, env):
    if env['PLATF'] != 'host':
        env = env.clone()
        env['CXXFLAGS'] += ['-fno-exceptions']
        # disable lto for gem5 for now, since it doesn't workhere ('plugin needed to handle lto object')
        # I don't know why it works for libm3, but not for libc.
        env.remove_flag('CXXFLAGS', '-flto')

        lib = env.static_lib(
            gen,
            out = 'libc',
            ins = env.glob('*/*.cc') + ['arch/' + env['ISA'] + '/PseudoInst.S']
        )
        env.install(gen, env['LIBDIR'], lib)
