def build(gen, env):
    if env['ISA'] == 'riscv':
        env.m3_rust_exe(gen, out='simplebench')
