from subprocess import Popen, PIPE


def build(gen, env):
    if env.try_execute('tud-otfconfig'):
        env = env.clone()

        p = Popen(['tud-otfconfig', '--includes', '--libs'], stdout=PIPE, stderr=PIPE)
        stdout, stderr = p.communicate()

        stderr = stderr.decode('utf-8')
        if stderr != '':
            print(stderr)

        libs = []
        for flag in stdout.decode('utf-8').split():
            if flag.startswith('-I'):
                env['CPPPATH'] += [flag[2:]]
            elif flag.startswith('-L'):
                env['LIBPATH'] += [flag[2:]]
            elif flag.startswith('-l'):
                libs += [flag[2:]]
            else:
                print('tud-otfconfig: unknown flag "%s"' % flag)

        bin = env.cxx_exe(gen, out='gem52otf', ins=['gem52otf.cc', 'Symbols.cc'], libs=libs)
        env.install(gen, env['TOOLDIR'], bin)
    else:
        print('Cannot execute tud-otfconfig, skipping gem52otf...')
