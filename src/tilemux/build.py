def build(gen, env):
    env = env.clone()

    env['LINKFLAGS'] += ['-nostartfiles']
    if env['ISA'] == 'arm':
        env['LINKFLAGS'] += ['-Wl,--whole-archive', '-lisr', '-Wl,--no-whole-archive']

    entry_file = 'src/arch/' + env['ISA'] + '/Entry.S'
    entry = env.asm(gen, out = entry_file[:-2] + '.o', ins = [entry_file])

    env.m3_rust_exe(
        gen,
        out = 'tilemux',
        libs = ['isr'],
        dir = None,
        ldscript = 'tilemux',
        startup = entry,
        varAddr = False
    )
