def build(gen, env):
    env.m3_rust_exe(gen, out = 'netrs', libs = ['thread', 'base', 'm3'])
