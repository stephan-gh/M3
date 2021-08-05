def build(gen, env):
    env = env.clone()

    env['CPPPATH'] += ['src/libs/flac/include']

    env.m3_exe(gen, out = 'imgsnd', ins = ['encoder.cc', 'imgsnd.cc'], libs = ['flac'])
