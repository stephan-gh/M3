dirs = [
    'allocator',
    'asciiplay',
    'bench',
    'coreutils',
    'disktest',
    'dosattack',
    'evilcompute',
    'faulter',
    'filterchain',
    'float',
    'hello',
    'netecho',
    'noop',
    'parchksum',
    'plasma',
    'queue',
    'rdwr',
    'rusthello',
    'ruststandalone',
    'rustunittests',
    'shell',
    'standalone',
    'timertest',
    'unittests',
    'vmtest',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
