def build(gen, env):
    if env['PLATF'] != 'host':
        lib = env.static_lib(
            gen,
            out = 'libgem5',
            ins = env.glob(env['ISA'] + '/*.*')
        )
        env.install(gen, env['LIBDIR'], lib)
