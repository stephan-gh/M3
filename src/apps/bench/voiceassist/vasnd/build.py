def build(gen, env):
    env = env.clone()

    env['CPPPATH'] += ['src/libs/flac/include']

    env.m3_exe(gen, out = 'vasnd', ins = env.glob('*.cc'), libs = ['flac'])
