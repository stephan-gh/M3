import os


def build(gen, env):
    env = env.clone()
    # tilemux has to use soft-float, because the applications might use the FPU and we have to make
    # sure to not overwrite the state (otherwise we would have to save&restore the complete state
    # on every entry and exit).
    env.soft_float()

    # use our own start file (Entry.S)
    env['LINKFLAGS'] += ['-nostartfiles']
    # for some reason, we need to force the linker to not throw away the isr_* functions on ARM
    if env['ISA'] == 'arm':
        env['LINKFLAGS'] += ['-Wl,--whole-archive', '-lisrsf', '-Wl,--no-whole-archive']

    entry_file = 'src/arch/' + env['ISA'] + '/Entry.S'
    entry = env.asm(gen, out=entry_file[:-2] + '.o', ins=[entry_file])

    # make sure that embedded C-code or similar (minicov with llvm-profile library) is build
    # with soft-float as well
    libs = ['isrsf']
    if env['ISA'] == 'riscv':
        cflags = '-march=rv64imac -mabi=lp64 '
        cflags += ' '.join(['-I' + os.path.abspath(i) for i in env['CPPPATH']])
        env['CRGENV']['TARGET_CFLAGS'] = cflags
    elif env['ISA'] == 'arm':
        libs += ['gcc_eh']

    # build tilemux outside of the workspace to use a different target spec that enables soft-float
    lib = env.m3_cargo(gen, out='libtilemux.a')
    env.install(gen, outdir=env['RUSTLIBS'], input=lib)

    # link it as usual
    env.m3_rust_exe(
        gen,
        out='tilemux',
        libs=libs,
        dir=None,
        ldscript='tilemux',
        startup=entry,
        varAddr=False
    )
