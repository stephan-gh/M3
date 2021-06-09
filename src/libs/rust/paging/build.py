def build(gen, env):
    if env['PLATF'] == 'kachel':
        env.m3_rust_lib(gen)
