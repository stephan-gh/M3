dirs = [
    'accelchain',
    'bench-apps',
    'compute',
    'cppbenchs',
    'fstrace',
    'imgproc',
    'loadgen',
    'netbandwidth',
    'netbandwidth_rs',
    'netfile',
    'netfileb',
    'netlatency',
    'netlatency_rs',
    'netstream',
    'rustbenchs',
    'rustnetbenchs',
    'scale',
    'scale-pipe',
    'tlbmiss',
    'vpe-clone',
    'vpe-exec',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
