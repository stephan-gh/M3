// vim:ft=cpp
/*
 * (c) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * (c) 2007-2013 Carsten Weinhold <weinhold@os.inf.tu-dresden.de>
 *     economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of TUD:OS, which is distributed under the terms of the
 * GNU General Public License 2. Please see the COPYING-GPL-2 file for details.
 */

#include "traceplayer.h"

#include <base/stream/Serial.h>
#include <base/time/Instant.h>

#include "buffer.h"
#include "exceptions.h"
#include "fsapi_m3fs.h"

using namespace m3;

__attribute__((unused)) static const char *op_names[] = {
    "INVALID", "WAITUNTIL", "OPEN",      "CLOSE",    "FSYNC",      "READ",   "WRITE",    "PREAD",
    "PWRITE",  "LSEEK",     "FTRUNCATE", "FSTAT",    "FSTATAT",    "STAT",   "RENAME",   "UNLINK",
    "RMDIR",   "MKDIR",     "SENDFILE",  "GETDENTS", "CREATEFILE", "ACCEPT", "RECVFROM", "WRITEV"};

int TracePlayer::play(Trace *trace, LoadGen::Channel *chan, bool data, bool stdio, bool keep_time,
                      bool verbose) {
    // determine max read and write buf size
    size_t rdBufSize = 0;
    size_t wrBufSize = 0;
    trace_op_t *op = trace->trace_ops;
    while(op && op->opcode != INVALID_OP) {
        switch(op->opcode) {
            case READ_OP:
            case PREAD_OP:
                rdBufSize = rdBufSize < op->args.read.size ? op->args.read.size : rdBufSize;
                break;
            case RECVFROM_OP:
                rdBufSize = rdBufSize < op->args.recvfrom.size ? op->args.recvfrom.size : rdBufSize;
                break;
            case WRITE_OP:
            case PWRITE_OP:
                wrBufSize = wrBufSize < op->args.write.size ? op->args.write.size : wrBufSize;
                break;
            case WRITEV_OP:
                wrBufSize = wrBufSize < op->args.writev.size ? op->args.writev.size : wrBufSize;
                break;
            case SENDFILE_OP:
                rdBufSize = rdBufSize < Buffer::MaxBufferSize ? Buffer::MaxBufferSize : rdBufSize;
                break;
        }
        op++;
    }

    Buffer buf(rdBufSize, wrBufSize);
    std::unique_ptr<FSAPI> fs(new FSAPI_M3FS(data, stdio, pathPrefix, chan));

    fs->start();

    CycleDuration wait_time;
    auto wait_start = CycleInstant::now();

    // let's play
    int lineNo = 1;
    op = trace->trace_ops;
    while(op && op->opcode != INVALID_OP) {
        auto start = CycleInstant::now();

        if(op->opcode != WAITUNTIL_OP)
            wait_time += CycleInstant::now().duration_since(wait_start);

        switch(op->opcode) {
            case WAITUNTIL_OP: {
                if(!keep_time)
                    break;

                fs->waituntil(&op->args.waituntil, lineNo);
                break;
            }
            case OPEN_OP: {
                fs->open(&op->args.open, lineNo);
                break;
            }
            case CLOSE_OP: {
                fs->close(&op->args.close, lineNo);
                break;
            }
            case FSYNC_OP: {
                fs->fsync(&op->args.fsync, lineNo);
                break;
            }
            case READ_OP: {
                read_args_t *args = &op->args.read;
                size_t amount = (stdio && args->fd == 0) ? static_cast<size_t>(args->err)
                                                         : args->size;
                for(unsigned int i = 0; i < args->count; i++) {
                    ssize_t err = fs->read(args->fd, buf.readBuffer(amount), amount);
                    if(err != (ssize_t)args->err)
                        throw ReturnValueException(err, args->err, lineNo);
                }
                break;
            }
            case WRITE_OP: {
                write_args_t *args = &op->args.write;
                size_t amount = (stdio && args->fd == 1) ? static_cast<size_t>(args->err)
                                                         : args->size;
                for(unsigned int i = 0; i < args->count; i++) {
                    ssize_t err = fs->write(args->fd, buf.writeBuffer(amount), amount);
                    if(err != (ssize_t)args->err)
                        throw ReturnValueException(err, args->err, lineNo);
                }
                break;
            }
            case PREAD_OP: {
                pread_args_t *args = &op->args.pread;
                ssize_t err =
                    fs->pread(args->fd, buf.readBuffer(args->size), args->size, args->offset);
                if(err != (ssize_t)args->err)
                    throw ReturnValueException(err, args->err, lineNo);
                break;
            }
            case PWRITE_OP: {
                pwrite_args_t *args = &op->args.pwrite;
                ssize_t err =
                    fs->pwrite(args->fd, buf.writeBuffer(args->size), args->size, args->offset);
                if(err != (ssize_t)args->err)
                    throw ReturnValueException(err, args->err, lineNo);
                break;
            }
            case LSEEK_OP: {
                fs->lseek(&op->args.lseek, lineNo);
                break;
            }
            case FTRUNCATE_OP: {
                fs->ftruncate(&op->args.ftruncate, lineNo);
                break;
            }
            case FSTAT_OP: {
                fs->fstat(&op->args.fstat, lineNo);
                break;
            }
            case FSTATAT_OP: {
                fs->fstatat(&op->args.fstatat, lineNo);
                break;
            }
            case STAT_OP: {
                fs->stat(&op->args.stat, lineNo);
                break;
            }
            case RENAME_OP: {
                fs->rename(&op->args.rename, lineNo);
                break;
            }
            case UNLINK_OP: {
                fs->unlink(&op->args.unlink, lineNo);
                break;
            }
            case RMDIR_OP: {
                fs->rmdir(&op->args.rmdir, lineNo);
                break;
            }
            case MKDIR_OP: {
                fs->mkdir(&op->args.mkdir, lineNo);
                break;
            }
            case SENDFILE_OP: {
                fs->sendfile(buf, &op->args.sendfile, lineNo);
                break;
            }
            case GETDENTS_OP: {
                fs->getdents(&op->args.getdents, lineNo);
                break;
            }
            case CREATEFILE_OP: {
                fs->createfile(&op->args.createfile, lineNo);
                break;
            }
            case ACCEPT_OP: {
                fs->accept(&op->args.accept, lineNo);
                break;
            }
            case RECVFROM_OP: {
                fs->recvfrom(buf, &op->args.recvfrom, lineNo);
                break;
            }
            case WRITEV_OP: {
                fs->writev(buf, &op->args.writev, lineNo);
                break;
            }
            default: {
                vthrow(Errors::NOT_SUP, "unsupported trace operation: {}"_cf, op->opcode);
            }
        }

        if(op->opcode != WAITUNTIL_OP)
            wait_start = CycleInstant::now();

        auto end = CycleInstant::now();
        if(verbose) {
            println("line {}: opcode={} -> {}"_cf, lineNo, op_names[op->opcode],
                    end.duration_since(start));
        }

        lineNo++;
        op++;
    }

    wait_time += CycleInstant::now().duration_since(wait_start);
    println("total waittime: {}"_cf, wait_time);
    fs->stop();
    return 0;
}
