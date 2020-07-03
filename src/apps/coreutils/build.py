dirs = [
    'cat',
    'cp',
    'echo',
    'ln',
    'ls',
    'mkdir',
    'paste',
    'rand',
    'readelf',
    'rm',
    'rmdir',
    'sink',
    'stat',
    'time',
    'tr',
    'wc',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
