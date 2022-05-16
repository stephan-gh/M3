def build(gen, env):
    bin = env.cargo(gen, out = 'netdbg')
    env.install(gen, env['TOOLDIR'], bin)
