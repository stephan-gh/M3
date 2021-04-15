dirs = [
    'allocator',
    'asciiplay',
    'bench',
    'coreutils',
    'cppnettests',
    'disktest',
    'dosattack',
    'evilcompute',
    'faulter',
    'filterchain',
    'float',
    'hello',
    'netechoserver',
    'noop',
    'parchksum',
    'plasma',
    'queue',
    'rdwr',
    'rusthello',
    'rustnettests',
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
