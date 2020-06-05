import os, sys
sys.path.insert(0, 'src/tools')
import install
import SCons

target = os.environ.get('M3_TARGET')
if target == 'gem5':
    isa = os.environ.get('M3_ISA', 'x86_64')

    if isa == 'arm':
        rustabi = 'gnueabihf'
        cross   = 'arm-none-eabi-'
        crt1    = 'crti.o'
    elif isa == 'riscv':
        rustabi = 'gnu'
        cross   = 'riscv64-unknown-elf-'
        crt1    = 'crti.o'
    else:
        rustabi = 'gnu'
        cross   = 'x86_64-elf-m3-'
        crt1    = 'crt1.o'
    crossdir    = Dir('build/cross-' + isa).abspath
    crossver    = '9.1.0'
    configpath  = Dir('.')
else:
    # build for host by default
    isa = os.popen("uname -m").read().strip()
    if isa == 'armv7l':
        isa = 'arm'

    target      = 'host'
    rustabi     = 'gnu'
    cross       = ''
    configpath  = Dir('.')

# build basic environment
baseenv = Environment(
    CPPFLAGS = '-D__' + target + '__',
    CXXFLAGS = ' -std=c++14 -Wall -Wextra -Wsign-conversion -fdiagnostics-color=always',
    CFLAGS   = ' -std=c99 -Wall -Wextra -Wsign-conversion',
    CPPPATH  = ['#src/include'],
)

vars = [
    'PATH',
    # required for colored outputs
    'HOME', 'TERM',
    # rust env vars (set in b)
    'RUST_TARGET_PATH', 'CARGO_TARGET_DIR', 'XBUILD_SYSROOT_PATH'
]
try:
    for v in vars:
        baseenv.Append(ENV = {v : os.environ[v]})
except KeyError as e:
    exit('Environment variable not found (' + str(e) + '). Please build M³ via ./b')

# hardlink support
link_builder = Builder(action = Action("ln -f ${SOURCE.abspath} ${TARGET.abspath}", "$LNCOMSTR"))
baseenv.Append(BUILDERS = {"Hardlink" : link_builder})

# for host compilation
hostenv = baseenv.Clone()
hostenv.Append(
    CPPFLAGS = ' -D__tools__',
)

# for target compilation
env = baseenv.Clone()
triple = isa + '-unknown-' + target + '-' + rustabi
env.Append(
    CXXFLAGS = ' -ffreestanding -fno-strict-aliasing -gdwarf-2 -fno-omit-frame-pointer' \
        ' -fno-threadsafe-statics -fno-stack-protector -Wno-address-of-packed-member',
    CPPFLAGS = ' -U_FORTIFY_SOURCE',
    CFLAGS = ' -gdwarf-2 -fno-stack-protector',
    ASFLAGS = ' -Wl,-W -Wall -Wextra',
    LINKFLAGS = ' -Wl,--no-gc-sections -Wno-lto-type-mismatch -fno-stack-protector',
    TRIPLE = triple,
    CRGFLAGS = '',
)

# add target-dependent stuff to env
if target == 'gem5':
    if isa == 'x86_64':
        # disable red-zone for all applications, because we used the application's stack in rctmux's
        # IRQ handlers since applications run in privileged mode. TODO can we enable that now?
        env.Append(CFLAGS = ' -mno-red-zone')
        env.Append(CXXFLAGS = ' -mno-red-zone')
    elif isa == 'arm':
        env.Append(CFLAGS = ' -march=armv7-a')
        env.Append(CXXFLAGS = ' -march=armv7-a')
        env.Append(LINKFLAGS = ' -march=armv7-a')
        env.Append(ASFLAGS = ' -march=armv7-a')
    elif isa == 'riscv':
        env.Append(CFLAGS = ' -march=rv64imafdc -mabi=lp64')
        env.Append(CXXFLAGS = ' -march=rv64imafdc -mabi=lp64')
        env.Append(LINKFLAGS = ' -march=rv64imafdc -mabi=lp64')
        env.Append(ASFLAGS = ' -march=rv64imafdc -mabi=lp64')
    env.Append(CPPPATH = [
        '#src/include/c',
        crossdir + '/include/c++/' + crossver,
        crossdir + '/include/c++/' + crossver + '/' + cross[:-1],
    ])
    # we install the crt* files to that directory
    env.Append(SYSGCCLIBPATH = Dir(crossdir + '/lib/gcc/' + cross[:-1] + '/' + crossver))
    # no build-id because it confuses gem5
    env.Append(LINKFLAGS = ' -static -Wl,--build-id=none')
    # binaries get very large otherwise
    env.Append(LINKFLAGS = ' -Wl,-z,max-page-size=4096 -Wl,-z,common-page-size=4096')
    # add cross-compiler binary dir to PATH
    env['ENV']['PATH'] = crossdir + '/bin:' + env['ENV']['PATH']

env.Replace(CXX = cross + 'g++')
env.Replace(AS = cross + 'gcc')
env.Replace(CC = cross + 'gcc')
env.Replace(LD = cross + 'ld')
env.Replace(AR = cross + 'gcc-ar')
env.Replace(RANLIB = cross + 'gcc-ranlib')

# add build-dependent flags (debug/release)
btype = os.environ.get('M3_BUILD')
if btype == 'debug':
    env.Append(CXXFLAGS = ' -O0 -g')
    env.Append(CFLAGS = ' -O0 -g')
    if target == 'host':
        env.Append(CXXFLAGS = ' -fsanitize=address -fsanitize=undefined')
        env.Append(CFLAGS = ' -fsanitize=address -fsanitize=undefined')
        env.Append(LINKFLAGS = ' -fsanitize=address -fsanitize=undefined -lasan -lubsan')
    env.Append(ASFLAGS = ' -g')
    hostenv.Append(CXXFLAGS = ' -O0 -g')
    hostenv.Append(CFLAGS = ' -O0 -g')
else:
    env.Append(CRGFLAGS = ' --release')
    env.Append(CXXFLAGS = ' -O2 -DNDEBUG -flto')
    env.Append(CFLAGS = ' -O2 -DNDEBUG -flto')
    env.Append(LINKFLAGS = ' -O2 -flto')
builddir = 'build/' + target + '-' + isa + '-' + btype

env.Append(CPPFLAGS = ' -DBUILD_DIR=' + builddir)

# add some important paths
env.Append(
    ARCH = target,
    ISA = isa,
    BUILD = btype,
    CFGS = configpath,
    BUILDDIR = Dir(builddir),
    BINARYDIR = Dir(builddir + '/bin'),
    LIBDIR = Dir(builddir + '/bin'),
    MEMDIR = Dir(builddir + '/mem'),
    FSDIR = Dir(builddir + '/fsdata'),
)
hostenv.Append(
    BUILD = btype,
    TOOLDIR = Dir(builddir + '/tools'),
    BINARYDIR = env['BINARYDIR'],
)

env.SConsignFile(builddir + '/.sconsign')
hostenv.SConsignFile(builddir + '/.sconsign')

def M3Config(env, custom_tests={}):
    return Configure(
        env,
        conf_dir='#' + builddir,
        log_file='#' + builddir + '/config.log',
        custom_tests=custom_tests
    )
env.AddMethod(M3Config)
hostenv.AddMethod(M3Config)

# check for tools
def CheckOTFConfig(context):
    context.Message('Checking for tud-otfconfig...')
    result = context.TryAction('tud-otfconfig')[0]
    context.Result(result)
    return result

def CheckRust(context):
    context.Message('Checking for cargo-xbuild...')
    result = context.TryAction('cargo xbuild -h')[0]
    context.Result(result)
    return result

conf = env.M3Config(custom_tests={
    'CheckOTFConfig': CheckOTFConfig,
    'CheckRust': CheckRust,
})
if not conf.CheckRust():
    exit('\033[1mYou need Rust including cargo-xbuild to build M³. See README.md.\033[0m')
hostenv['HAS_OTF']  = conf.CheckOTFConfig()
conf.Finish()

def M3Mkfs(env, target, source, blocks, inodes):
    fs = env.Command(
        target, source,
        Action(
            '$BUILDDIR/src/tools/mkm3fs/mkm3fs $TARGET $SOURCE %d %d 0' % (blocks, inodes),
            '$MKFSCOMSTR'
        )
    )
    env.Depends(fs, '$BUILDDIR/src/tools/mkm3fs/mkm3fs')
    env.Hardlink('$BUILDDIR/' + fs[0].name, fs)

def M3Strip(env, target, source):
    return env.Command(
        target, source,
        Action(
            cross + 'strip -o $TARGET $SOURCE',
            '$STRIPCOMSTR'
        )
    )

def M3CPP(env, target, source):
    env.Command(
        target, source,
        Action(
            cross + 'cpp -P $CPPFLAGS $SOURCE -o $TARGET',
            '$CPPCOMSTR'
        )
    )

def_ldscript = env.File('$BUILDDIR/ld-default.conf')
M3CPP(env, def_ldscript, '#src/toolchain/gem5/ld.conf')

isr_ldscript = env.File('$BUILDDIR/ld-isr.conf')
myenv = env.Clone()
myenv.Append(CPPFLAGS = ' -D__isr__=1')
M3CPP(myenv, isr_ldscript, '#src/toolchain/gem5/ld.conf')

link_addr = 0x220000

def M3Program(env, target, source, libs = [], NoSup = False, ldscript = None, varAddr = True):
    myenv = env.Clone()

    m3libs = ['base', 'm3', 'thread']

    if myenv['ARCH'] == 'gem5':
        if not NoSup:
            baselibs = ['gcc', 'c', 'm', 'gloss', 'stdc++', 'supc++', 'heap']
            if env['ISA'] == 'x86_64':
                baselibs += ['gcc_eh']
            libs = baselibs + m3libs + libs

        if ldscript is None:
            ldscript = isr_ldscript if 'isr' in libs else def_ldscript
        myenv.Append(LINKFLAGS = ' -Wl,-T,' + ldscript.abspath)

        if varAddr:
            global link_addr
            myenv.Append(LINKFLAGS = ' -Wl,--section-start=.text=' + ("0x%x" % link_addr))
            link_addr += 0x40000

        # search for crt* in our library dir
        myenv.Append(LINKFLAGS = ' -B' + env['LIBDIR'].abspath)

        # TODO workaround to ensure that our memcpy, etc. is used instead of the one from Rust's
        # compiler-builtins crate, because those are poor implementations. Note that we do that for
        # all M3 programs, because for example the kernel links against a Rust library.
        sources = [source] + myenv.Glob('$BUILDDIR/src/libs/c/string/*.o')

        prog = myenv.Program(
            target, sources,
            LIBS = libs,
            LIBPATH = [crossdir + '/lib', myenv['LIBDIR']]
        )
        myenv.Depends(prog, myenv.Glob('$LIBDIR/crt*.o'))
        myenv.Depends(prog, ldscript)
    else:
        if not NoSup:
            libs = m3libs + ['m3', 'heap', 'pthread'] + libs

        prog = myenv.Program(
            target, source,
            LIBS = libs,
            LIBPATH = [myenv['LIBDIR']]
        )

    myenv.Install(myenv['BINARYDIR'], prog)
    return prog

def Cargo(env, target, source):
    return env.Command(
        target, source,
        Action(
            'cd ${SOURCE.dir.dir} && cargo xbuild --target $TRIPLE $CRGFLAGS',
            '$CRGCOMSTR'
        )
    )

def RustLibrary(env, target):
    rustdir = env['ENV']['CARGO_TARGET_DIR']
    builddir = 'debug' if env['BUILD'] == 'debug' else 'release'
    stlib = env.Cargo(
        target = rustdir + '/$TRIPLE/' + builddir + '/lib' + target + '.a',
        source = 'src/' + target + '.rs'
    )
    env.Install(env['LIBDIR'], stlib)
    env.Depends(stlib, env.File('Cargo.toml'))
    env.Depends(stlib, env.File('#Cargo.toml'))
    env.Depends(stlib, env.File('#src/toolchain/rust/' + env['TRIPLE'] + '.json'))
    return stlib

def RustProgram(env, target, libs = [], startup = None, ldscript = None, varAddr = True):
    myenv = env.Clone()
    myenv.Append(LINKFLAGS = ' -Wl,-z,muldefs')
    stlib = RustLibrary(myenv, target)

    if myenv['ARCH'] == 'gem5':
        sources = [myenv['LIBDIR'].abspath + '/crt0.o' if startup is None else startup]
        libs    = ['c', 'm', 'gloss', 'heap', 'gcc', target] + libs
    else:
        sources = []
        # leave the host lib in here as well to let scons know about the dependency
        libs    = ['c', 'heap', 'host', 'gcc', 'pthread', target] + libs
        # ensure that the host library gets linked in
        myenv.Append(LINKFLAGS = ' -Wl,--whole-archive -lhost -Wl,--no-whole-archive')

    prog = myenv.M3Program(
        myenv,
        target = target,
        source = sources,
        libs = libs,
        NoSup = True,
        ldscript = ldscript,
        varAddr = varAddr,
    )
    if not startup is None:
        myenv.Depends(prog, startup)
    return prog

env.AddMethod(Cargo)
env.AddMethod(M3Mkfs)
env.AddMethod(M3Strip)
env.AddMethod(M3CPP)
env.AddMethod(install.InstallFiles)
env.M3Program = M3Program
env.RustProgram = RustProgram
env.RustLibrary = RustLibrary

# always use grouping for static libraries, because they may depend on each other so that we want
# to cycle through them until all references are resolved.
env['_LIBFLAGS'] = '-Wl,--start-group ' + env['_LIBFLAGS'] + ' -Wl,--end-group'

env.SConscript('src/SConscript', exports = ['env', 'hostenv'], variant_dir = builddir, src_dir = '.', duplicate = 0)

if ARGUMENTS.get('dump_trace', 0):
    env.SetOption('no_exec', True)
    env.Decider(lambda x, y, z: True)
    SCons.Node.Python.Value.changed_since_last_build = (lambda x, y, z: True)
