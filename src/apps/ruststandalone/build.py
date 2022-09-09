def build(gen, env):
    if env['ISA'] == 'riscv':
        for d in ['stdasender', 'stdareceiver', 'vmtest']:
            env.sub_build(gen, d)
