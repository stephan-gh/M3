def build(gen, env):
    if env['PLATF'] != 'host':
        env.m3_exe(gen, out = 'libctest', ins = ['libctest.cc'] + env.glob('tests/*.cc'))
