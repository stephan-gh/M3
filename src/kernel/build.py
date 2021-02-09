def build(gen, env):
    env = env.clone()

    if env['ISA'] == 'arm':
        env['LINKFLAGS'] += ['-Wl,--whole-archive', '-lisr', '-Wl,--no-whole-archive']

    libs = ['isr', 'thread'] if env['PLATF'] == 'kachel' else ['thread']
    env.m3_rust_exe(gen, out = 'kernel', libs = libs, ldscript = 'isr', varAddr = False)
