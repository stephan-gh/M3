def build(gen, env):
    env.build_fs(gen, out = 'bench.img', dir = '.', blocks = 64 * 1024, inodes = 4096)
