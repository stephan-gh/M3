def build(gen, env):
    env.build_fs(gen, out = 'default.img', dir = '.', blocks = 32 * 1024, inodes = 512)
