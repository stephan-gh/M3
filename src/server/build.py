dirs = [
    'arith',
    'console',
    'disk',
    'm3fs',
    'netrs',
    'pager',
    'pipes',
    'root',
    'timer',
    'vterm',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
