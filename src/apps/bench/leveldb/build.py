def build(gen, env):
    # not supported on host; too big for the SPM in debug mode
    if env['PLATF'] == 'host' or (env['TGT'] == 'hw' and env['BUILD'] == 'debug'):
        return

    env = env.clone()
    env['CPPPATH'] += ['src/libs/leveldb/include']
    env.m3_exe(gen, out = 'leveldb', libs = ['leveldb'], ins = ['leveldb.cc'])
