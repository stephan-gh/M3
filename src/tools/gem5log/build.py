def build(gen, env):
    bin = env.m3_cargo(gen, out = 'gem5log')
    env.install(gen, env['TOOLDIR'], bin)
