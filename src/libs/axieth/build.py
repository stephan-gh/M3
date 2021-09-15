import os

def build(gen, env):
    if env['TGT'] == 'hw':
        env = env.clone()

        pwd = str(env.glob('')[0])
        env['CPPPATH'] += [pwd  + 'common', pwd + 'llfifo', pwd + 'axidma', pwd + 'axiethernet']
        
        if os.environ.get('M3_BUILD') == 'debug':
            env['CXXFLAGS'] += ['-DDEBUG', '-UNDEBUG']
            env['CFLAGS'] += ['-DDEBUG', '-UNDEBUG']
        # else:
        #     env['CXXFLAGS'] += ['-DDEBUG']
        #     env['CFLAGS'] += ['-DDEBUG']

        env['CXXFLAGS']  += ['-fno-exceptions']
        env['LINKFLAGS'] += ['-fno-exceptions', '-nodefaultlibs']

        env['CXXFLAGS'] += [
            '-Wno-sign-conversion',
            '-Wno-unused-parameter',
            '-Wno-unused-function',
            '-Wno-unused-but-set-variable',
        ]

        env_obj = env.cxx(gen, out = 'env.o', ins = ['env.cc'])
        files = [env_obj, 'axieth.cc'] + env.glob('common/*.cc') + env.glob('llfifo/*.cc') + env.glob('axidma/*.cc') + env.glob('axiethernet/*.cc')
        lib = env.static_lib(gen, out = 'libaxieth', ins = files)
        env.install(gen, env['LIBDIR'], lib)

        env.m3_exe(
            gen,
            out = 'axi_ethernet_driver',
            ins = [
                'axi_ethernet_driver.cc', 'xaxiethernet_example_util.cc',
                'xaxiethernet_example_polled.cc', 'xaxiethernet_example_intr_fifo.cc',
                'xaxiethernet_fifo_ping_req_example.cc',
                'xaxiethernet_example_sgdma_poll.cc'
            ],
            dir = None,
            NoSup = True,
            ldscript = 'baremetal',
            varAddr = False,
            libs = ['simplec', 'gem5', 'heap', 'base', 'supc++', 'gcc', 'axieth']
        )
