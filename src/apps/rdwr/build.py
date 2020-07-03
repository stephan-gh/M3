def build(gen, env):
    env.m3_exe(gen, out = 'rdwr', ins = ['rdwr.cc'])
