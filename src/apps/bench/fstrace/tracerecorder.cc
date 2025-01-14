// vim:ft=cpp
/*
 * (c) 2007-2013 Carsten Weinhold <weinhold@os.inf.tu-dresden.de>
 *     economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of TUD:OS, which is distributed under the terms of the
 * GNU General Public License 2. Please see the COPYING-GPL-2 file for details.
 */

#include "tracerecorder.h"

#include "exceptions.h"
#include "opdescr.h"

using namespace std;

void TraceRecorder::print(const char *name) {
    unsigned int lineNo = 1;

    printPrologue(name);

    TraceListIterator i = ops.begin();
    while(i != ops.end()) {
        cout << (*i)->codeLine(lineNo) << endl;

        ++i;
        lineNo++;
    }

    printEpilogue();
}

void TraceRecorder::import() {
    FoldableOpDescr *lastFod = 0;
    unsigned int lineNo = 1;

    while(cin.good()) {
        string line;
        char buffer[4096];

        cin.getline(buffer, sizeof(buffer));
        if(cin.eof())
            break;
        if(cin.bad() || cin.fail())
            throw IoException("read", "stdin", -1);

        line = buffer;

        OpDescr *od = OpDescrFactory::create(line);
        if(od) {
            FoldableOpDescr *fod = dynamic_cast<FoldableOpDescr *>(od);

            if(lastFod && fod && lastFod->merge(*fod))
                delete od;

            else {
                lastFod = (fod) ? fod : nullptr;
                ops.push_back(od);
            }
        }
        else
            memorizeUnkownSysCall(OpDescrFactory::sysCallName(line));

        lineNo++;
    }

    reportUnknownSysCalls();
}

void TraceRecorder::printPrologue(const char *name) {
    cout << "// This file has been automatically generated by strace2c." << endl;
    cout << "// Do not edit it!" << endl << endl;
    cout << "#include \"../op_types.h\"" << endl << endl;
    cout << "trace_op_t trace_ops_" << name << "[] = {" << endl;
}

void TraceRecorder::printEpilogue() {
    cout << "    { .opcode = INVALID_OP } " << endl;
    cout << "};" << endl;
}

void TraceRecorder::memorizeUnkownSysCall(const string &sysCallName) {
    sysCalls.insert(sysCallName);
}

void TraceRecorder::reportUnknownSysCalls() {
    if(sysCalls.empty())
        return;

    cerr << "Ignored the following system calls:" << endl;

    SysCallSetIterator i = sysCalls.begin();
    while(i != sysCalls.end()) {
        cerr << "    " << (*i) << "()" << endl;
        ++i;
    }
}
