def build(gen, env):
    bin = env.cargo(gen, out = 'hwitrace')
    env.install(gen, env['TOOLDIR'], bin)
