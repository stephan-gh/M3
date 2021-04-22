def build(gen, env):
    if env['PLATF'] == 'kachel':
        env = env.clone()

        env['LINKFLAGS'] += ['-nostartfiles']

        entry_file = 'src/arch/' + env['ISA'] + '/Entry.S'
        entry = env.asm(gen, out = entry_file[:-2] + '.o', ins = [entry_file])

        env.m3_rust_exe(
            gen,
            out = 'pemux',
            libs = ['isr'],
            ldscript = 'pemux',
            startup = entry,
            varAddr = False
        )
