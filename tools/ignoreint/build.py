def build(gen, env):
    bin = env.cxx_exe(gen, out='ignoreint', ins=['ignoreint.cc'])
    env.install(gen, env['TOOLDIR'], bin)
