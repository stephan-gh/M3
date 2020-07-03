def build(gen, env):
    env.m3_exe(gen, out = 'rmdir', ins = ['rmdir.cc'])
