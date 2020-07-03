def build(gen, env):
    env = env.clone()
    env['CXXFLAGS'] += ['-fno-exceptions']
    env.remove_flag('CXXFLAGS', '-flto')
    lib = env.static_lib(
        gen,
        out = 'libisr',
        ins = [
            'arch/' + env['ISA'] + '/Entry.S',
            'arch/' + env['ISA'] + '/ISR.cc',
            'ISR.cc',
        ]
    )
    env.install(gen, env['LIBDIR'], lib)
