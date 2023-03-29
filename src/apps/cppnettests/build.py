def build(gen, env):
    env.m3_exe(gen, out='cppnettests', ins=['cppnettests.cc'] + env.glob(gen, 'tests/*.cc'))
