def build(gen, env):
    env = env.clone()

    if env['ISA'] == 'arm':
        env['LINKFLAGS'] += ['-Wl,--whole-archive', '-lisr', '-Wl,--no-whole-archive']

    env.m3_rust_exe(
        gen,
        out='vmtest',
        libs=['isr'],
        dir=None,
        ldscript='tilemux',
        varAddr=False,
        features=["vmtest/" + env['TGT']],
    )
