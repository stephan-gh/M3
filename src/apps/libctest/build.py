def build(gen, env):
    env.m3_exe(gen, out='libctest', ins=['libctest.cc'] + env.glob(gen, 'tests/*.cc'))
