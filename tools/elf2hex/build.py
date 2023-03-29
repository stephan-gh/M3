def build(gen, env):
    bin = env.cxx_exe(gen, out='elf2hex', ins=['elf2hex.cc'])
    env.install(gen, env['TOOLDIR'], bin)
