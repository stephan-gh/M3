dirs = [
    'base',
    'c',
    'dummy',
    'heap',
    'host',
    'm3',
    'pci',
    'rust',
    'support',
    'thread',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
