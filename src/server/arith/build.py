def build(gen, env):
    env.m3_exe(gen, out='arith', ins=['arith.cc'], dir='sbin')
