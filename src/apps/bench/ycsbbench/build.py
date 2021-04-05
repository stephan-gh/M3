dirs = [
    'netrs_ycsb_bench_client',
    'netrs_ycsb_bench_server',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
