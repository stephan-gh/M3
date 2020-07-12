def build(gen, env):
    lib = env.static_lib(
        gen,
        out = 'libisr',
        ins = ['src/' + env['ISA'] + '/Entry.S']
    )
    env.install(gen, env['LIBDIR'], lib)
