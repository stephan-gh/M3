dirs = [
    'netrs_bandwidth_client',
    'netrs_bandwidth_server',
    'netrs_latency_client',
    'netrs_latency_server',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
