def build(gen, env):
    if env['PLATF'] == 'kachel' and env['ISA'] == 'riscv':
        for d in ['stdasender', 'stdareceiver', 'vmtest']:
            env.sub_build(gen, d)
