def build(gen, env):
    bin = env.cxx_exe(gen, out='m3fsck', ins=['m3fsck.cc'])
    env.install(gen, env['TOOLDIR'], bin)
