def build(gen, env):
    env = env.clone()
    env['CPPPATH'] += ['src/libs/jpeg']
    env.m3_exe(gen, out = 'jpegtest', ins = ['jpegtest.cc'], libs = ['jpeg'])
