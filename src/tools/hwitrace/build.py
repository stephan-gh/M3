def build(gen, env):
    bin = env.m3_cargo(gen, out = 'hwitrace')
    env.install(gen, env['TOOLDIR'], bin)
