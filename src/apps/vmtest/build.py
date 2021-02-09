def build(gen, env):
    if env['PLATF'] == 'kachel' and env['ISA'] == 'riscv':
        env.m3_rust_exe(gen, out = 'vmtest', libs = ['isr'], ldscript = 'pemux', varAddr = False)
