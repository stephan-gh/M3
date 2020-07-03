def build(gen, env):
    env.m3_exe(gen, out = 'stat', ins = ['stat.cc'])
