dirs = [
    'base',
    'c',
    'dummy',
    'heap',
    'host',
    'isr',
    'm3',
    'pci',
    'support',
    'thread',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
