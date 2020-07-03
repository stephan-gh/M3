import src.tools.ninjagen as ninjagen

def build(gen, env):
    if env['TGT'] == 'gem5' or env['TGT'] == 'host':
        env = env.clone()
        env['CPPPATH'] += [
            ninjagen.SourcePath.new(env, 'lwip/include'),
            ninjagen.SourcePath.new(env, 'lwip/port/include'),
        ]

        lwenv = env.clone()
        # silence warnings in lwip code
        lwenv['CFLAGS'] += ['-Wno-sign-conversion']
        lwsrc = env.glob('lwip/api/*.c') + \
                env.glob('lwip/core/*.c') + \
                env.glob('lwip/core/ipv4/*.c') + \
                env.glob('lwip/core/ipv6/*.c') + \
                env.glob('lwip/netif/*.c') + \
                env.glob('lwip/port/*.c')
        lwobj = [lwenv.cc(gen, out = f[:-2] + '.o', ins = [f]) for f in lwsrc]
        lwobj += [env.cxx(gen, out = f[:-3] + '.o', ins = [f]) for f in env.glob('lwip/port/*.cc')]

        env.m3_exe(
            gen,
            out = 'net',
            ins = env.glob('*.cc') + \
                  env.glob('driver/' + env['TGT'] + '/*.cc') + \
                  env.glob('sess/*.cc') + \
                  env.glob('sess/socket/*.cc') + \
                  lwobj,
            libs = ['pci'] if env['TGT'] == 'gem5' else [],
        )
