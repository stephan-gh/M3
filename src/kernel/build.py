def build(gen, env):
    env = env.clone()

    if env['ISA'] == 'arm':
        env['LINKFLAGS'] += ['-Wl,--whole-archive', '-lisr', '-Wl,--no-whole-archive']

    env.m3_rust_exe(
        gen, out='kernel', libs=['isr', 'thread'], dir=None, ldscript='isr', varAddr=False,
        features=["kernel/" + env['TGT']]
    )
