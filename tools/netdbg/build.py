def build(gen, env):
    bin = env.rust_exe(gen, out='netdbg')
    env.install(gen, env['TOOLDIR'], bin)
