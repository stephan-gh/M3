def build(gen, env):
    bin = env.rust_exe(gen, out='gem5log')
    env.install(gen, env['TOOLDIR'], bin)
