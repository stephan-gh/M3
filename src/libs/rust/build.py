dirs = [
    'base',
    'heap',
    'isr',
    'm3',
    'paging',
    'pci',
    'resmng',
    'thread',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
