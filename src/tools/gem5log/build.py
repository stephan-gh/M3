def build(gen, env):
    bin = env.cargo(gen, out = 'gem5log')
    env.install(gen, env['TOOLDIR'], bin)
