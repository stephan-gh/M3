def build(gen, env):
    if env['PLATF'] == 'host':
        return

    env = env.clone()
    env['CPPPATH'] += ['src/libs/leveldb/include']
    env.m3_exe(gen, out = 'leveldb', libs = ['leveldb'], ins = ['leveldb.cc'])
