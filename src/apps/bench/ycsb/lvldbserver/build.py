def build(gen, env):
    # not supported on host
    if env['PLATF'] == 'host':
        return

    env = env.clone()
    env['CPPPATH'] += ['src/libs/leveldb/include']
    env.m3_exe(gen, out = 'lvldbserver', libs = ['leveldb'], ins = env.glob('*.cc'))
