def build(gen, env):
    if env['PLATF'] == 'kachel':
        env = env.clone()
        env['CXXFLAGS']  += ['-fno-exceptions']
        env['LINKFLAGS'] += ['-fno-exceptions']

        env_obj = env.cxx(gen, out = 'env.o', ins = ['env.cc'])
        env.m3_exe(
            gen,
            out = 'standalone',
            ins = [env_obj, 'standalone.cc'],
            libs = ['c', 'heap', 'base', 'supc++'],
            ldscript = 'isr',
            NoSup = True
        )

        for s in ['sender', 'receiver']:
            env.m3_exe(
                gen,
                out = 'standalone-' + s,
                ins = [env_obj, s + '/' + s + '.cc'],
                NoSup = True,
                ldscript = 'isr',
                libs = ['c', 'heap', 'base', 'supc++']
            )
