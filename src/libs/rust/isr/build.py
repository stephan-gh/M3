def build(gen, env):
    if env['PLATF'] == 'kachel':
        env.m3_rust_lib(gen)
        lib = env.static_lib(
            gen,
            out = 'libisr',
            ins = ['src/' + env['ISA'] + '/Entry.S']
        )
        env.install(gen, env['LIBDIR'], lib)
