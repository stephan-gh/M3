def build(gen, env):
    if env['PLATF'] == 'kachel':
        env = env.clone()
        env['CXXFLAGS']  += ['-fno-exceptions']
        env['LINKFLAGS'] += ['-fno-exceptions', '-nodefaultlibs']

        libs = ['simplec', 'gem5', 'heap', 'base', 'supc++', 'gcc']
        if env['ISA'] == 'x86_64':
            libs += ['gcc_eh']

        env_obj = env.cxx(gen, out = 'env.o', ins = ['env.cc'])
        env.m3_exe(
            gen,
            out = 'standalone',
            ins = [env_obj, 'standalone.cc'] + env.glob('tests/*.cc'),
            libs = libs,
            ldscript = 'baremetal',
            NoSup = True
        )

        for s in ['sender', 'receiver', 'mem']:
            env.m3_exe(
                gen,
                out = 'standalone-' + s,
                ins = [env_obj, s + '/' + s + '.cc'],
                NoSup = True,
                ldscript = 'baremetal',
                libs = libs
            )
