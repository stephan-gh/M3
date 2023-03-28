def build(gen, env):
    bin = env.m3_cargo(gen, out = 'netdbg')
    env.install(gen, env['TOOLDIR'], bin)
