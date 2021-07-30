dirs = [
    'axieth',
    'base',
    'dummy',
    'gem5',
    'heap',
    'host',
    'jpeg',
    'leveldb',
    'm3',
    'memory',
    'musl',
    'pci',
    'rust',
    'support',
    'thread',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
