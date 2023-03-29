def build(gen, env):
    bin = env.rust_exe(gen, out='hwitrace')
    env.install(gen, env['TOOLDIR'], bin)
