def build(gen, env):
    env.m3_exe(gen, out='cppbenchs', ins=['cppbenchs.cc'] + env.glob(gen, 'benchs/*.cc'))
