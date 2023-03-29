def build(gen, env):
    env.m3_exe(gen, out='cppnetbenchs', ins=['cppnetbenchs.cc'] + env.glob(gen, 'benchs/*.cc'))
