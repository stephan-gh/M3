/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

#include <base/Common.h>

#include <m3/stream/FStream.h>
#include <m3/stream/Standard.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/vfs/VFS.h>
#include <m3/vfs/FileRef.h>
#include <m3/vfs/Dir.h>
#include <m3/Test.h>

#include "../unittests.h"

using namespace m3;

static uint8_t largebuf[100 * 8];

static const char *small_file = "/test.txt";
static const char *pat_file = "/pat.bin";

static void check_content(const char *filename, size_t size) {
    FileRef file(filename, FILE_R);

    size_t pos = 0;
    ssize_t count;
    while((count = file->read(largebuf, sizeof(largebuf))) > 0) {
        for(ssize_t i = 0; i < count; ++i) {
            if(largebuf[i] != pos % 100) {
                cout << "file[" << pos << "]: expected "
                     << (pos % 100) << ", got " << largebuf[i] << "\n";
                WVASSERT(false);
            }
            pos++;
        }
    }
    WVASSERTEQ(pos, size);

    FileInfo info;
    file->stat(info);
    WVASSERTEQ(info.size, size);
}

static void append_bug() {
    size_t total = 0;

    {
        FileRef file("/myfile1", FILE_W | FILE_CREATE | FILE_TRUNC);
        for(size_t i = 0; i < sizeof(largebuf); ++i)
            largebuf[i] = i % 100;

        // create first extent
        file->write_all(largebuf, sizeof(largebuf));
        file->flush();
        total += sizeof(largebuf);

        // use the following blocks for something else to force a new extent for the following write
        {
            FileRef nfile("/myfile2", FILE_W | FILE_CREATE | FILE_TRUNC);
            nfile->write_all(largebuf, sizeof(largebuf));
        }

        // write more two blocks; this gives us a new extent and we don't stay within the first block
        // of the new extent
        for(size_t i = 0; i <= 4096 * 2; i += sizeof(largebuf)) {
            file->write_all(largebuf, sizeof(largebuf));
            total += sizeof(largebuf);
        }
    }

    {
        FileRef file("/myfile1", FILE_W);
        file->seek(0, M3FS_SEEK_END);

        file->write_all(largebuf, sizeof(largebuf));
        total += sizeof(largebuf);
    }

    check_content("/myfile1", total);
}

static void extending_small_file() {
    {
        FileRef file(small_file, FILE_W);
        for(size_t i = 0; i < sizeof(largebuf); ++i)
            largebuf[i] = i % 100;

        for(int i = 0; i < 129; ++i)
            file->write_all(largebuf, sizeof(largebuf));
    }

    check_content(small_file, sizeof(largebuf) * 129);
}

static void creating_in_steps() {
    {
        FileRef file("/steps.txt", FILE_W | FILE_CREATE);
        for(size_t i = 0; i < sizeof(largebuf); ++i)
            largebuf[i] = i % 100;

        for(int j = 0; j < 8; ++j) {
            for(int i = 0; i < 4; ++i)
                file->write_all(largebuf, sizeof(largebuf));
            file->flush();
        }
    }

    check_content("/steps.txt", sizeof(largebuf) * 8 * 4);
}

static void small_write_at_begin() {
    {
        FileRef file(small_file, FILE_W);
        for(size_t i = 0; i < sizeof(largebuf); ++i)
            largebuf[i] = i % 100;

        for(int i = 0; i < 3; ++i)
            file->write_all(largebuf, sizeof(largebuf));
    }

    check_content(small_file, sizeof(largebuf) * 129);
}

static void truncate() {
    {
        FileRef file(small_file, FILE_W | FILE_TRUNC);
        for(size_t i = 0; i < sizeof(largebuf); ++i)
            largebuf[i] = i % 100;

        for(int i = 0; i < 2; ++i)
            file->write_all(largebuf, sizeof(largebuf));
    }

    check_content(small_file, sizeof(largebuf) * 2);
}

static void append() {
    {
        FileRef file(small_file, FILE_W | FILE_APPEND);
        for(size_t i = 0; i < sizeof(largebuf); ++i)
            largebuf[i] = i % 100;

        for(int i = 0; i < 2; ++i)
            file->write_all(largebuf, sizeof(largebuf));
    }

    check_content(small_file, sizeof(largebuf) * 4);
}

static void append_with_read() {
    {
        FileRef file(small_file, FILE_RW | FILE_TRUNC | FILE_CREATE);
        for(size_t i = 0; i < sizeof(largebuf); ++i)
            largebuf[i] = i % 100;

        for(int i = 0; i < 2; ++i)
            file->write_all(largebuf, sizeof(largebuf));

        // there is nothing to read now
        WVASSERTEQ(file->read(largebuf, sizeof(largebuf)), 0);

        // seek back
        WVASSERTEQ(file->seek(sizeof(largebuf) * 1, M3FS_SEEK_SET), sizeof(largebuf) * 1);
        // now reading should work
        WVASSERTEQ(file->read(largebuf, sizeof(largebuf)), static_cast<ssize_t>(sizeof(largebuf)));
    }

    check_content(small_file, sizeof(largebuf) * 2);
}

static void append_with_commit() {
    {
        FileRef file("/myfile", FILE_RW | FILE_TRUNC | FILE_CREATE);
        for(size_t i = 0; i < sizeof(largebuf); ++i)
            largebuf[i] = i % 100;

        // we assume a blocksize of 4096 here
        {
            FileInfo info;
            file->stat(info);
            WVASSERTEQ(info.blocksize, 4096u);
        }

        size_t off = 0;
        for(int i = 0; i < 2; ++i) {
            size_t rem = 4096;
            while(rem > 0) {
                size_t amount = std::min(rem, sizeof(largebuf) - off);
                file->write_all(largebuf + off, amount);
                off = (off + amount) % sizeof(largebuf);
                rem -= amount;
            }
            if(i == 0)
                file->flush();
        }
    }

    check_content("/myfile", 8192);
}

static void file_mux() {
    const size_t NUM = 2;
    const size_t STEP_SIZE = 400;
    const size_t FILE_SIZE = 12 * 1024;

    FStream *files[NUM];
    for(size_t i = 0; i < NUM; ++i)
        files[i] = new FStream(pat_file, FILE_R);

    for(size_t pos = 0; pos < FILE_SIZE; pos += STEP_SIZE) {
        for(size_t i = 0; i < NUM; ++i) {
            size_t tpos = pos;
            size_t end = Math::min(FILE_SIZE, pos + STEP_SIZE);
            while(tpos < end) {
                uint8_t byte = static_cast<uint8_t>(files[i]->read());
                WVASSERTEQ(byte, tpos & 0xFF);
                tpos++;
            }
        }
    }

    for(size_t i = 0; i < NUM; ++i)
        delete files[i];
}

static void pipe_mux() {
    const size_t NUM = 2;
    const size_t STEP_SIZE = 16;
    const size_t DATA_SIZE = 1024;
    const size_t PIPE_SIZE = 256;

    try {
        Pipes pipesrv("pipes");
        MemGate *mems[NUM];
        IndirectPipe *pipes[NUM];
        Reference<File> reader[NUM];
        Reference<File> writer[NUM];
        for(size_t i = 0; i < NUM; ++i) {
            mems[i] = new MemGate(MemGate::create_global(PIPE_SIZE, MemGate::RW));
            pipes[i] = new IndirectPipe(pipesrv, *mems[i], PIPE_SIZE);
            reader[i] = Activity::self().files()->get(pipes[i]->reader_fd());
            writer[i] = Activity::self().files()->get(pipes[i]->writer_fd());
        }

        char src_buf[STEP_SIZE];
        for(size_t i = 0; i < STEP_SIZE; ++i)
            src_buf[i] = 'a' + i;

        for(size_t pos = 0; pos < DATA_SIZE; pos += STEP_SIZE) {
            for(size_t i = 0; i < NUM; ++i) {
                writer[i]->write(src_buf, STEP_SIZE);
                writer[i]->flush();
            }

            for(size_t i = 0; i < NUM; ++i) {
                char dst_buf[STEP_SIZE];
                memset(dst_buf, 0, STEP_SIZE);

                reader[i]->read(dst_buf, STEP_SIZE);

                WVASSERTEQ(memcmp(src_buf, dst_buf, STEP_SIZE), 0);
            }
            pos += STEP_SIZE;
        }

        for(size_t i = 0; i < NUM; ++i) {
            delete pipes[i];
            delete mems[i];
        }
    }
    catch(const Exception &e) {
        cerr << "pipes test failed: " << e.what() << "\n";
    }
}

static void file_errors() {
    const char *filename = "/subdir/subsubdir/testfile.txt";

    char buf[8];
    {
        FileRef file(filename, FILE_R);
        WVASSERTERR(Errors::NO_PERM, [&file, &buf] {
            file->write(buf, sizeof(buf));
        });
    }

    {
        FileRef file(filename, FILE_W);
        WVASSERTERR(Errors::NO_PERM, [&file, &buf] {
            file->read(buf, sizeof(buf));
        });
    }
}

static void read_file_at_once() {
    const char *filename = "/subdir/subsubdir/testfile.txt";
    const char content[] = "This is a test!\n";
    char buf[sizeof(content)];

    FileRef file(filename, FILE_R);
    WVASSERTEQ(file->read(buf, sizeof(buf) - 1), static_cast<ssize_t>(sizeof(buf) - 1));
    buf[sizeof(buf) - 1] = '\0';

    WVASSERTEQ(buf, StringRef(content));
}

static void read_file_in_64b_steps() {
    FileRef file(pat_file, FILE_R);

    uint8_t buf[64];
    ssize_t count, pos = 0;
    while((count = file->read(buf, sizeof(buf))) > 0) {
        for(ssize_t i = 0; i < count; ++i)
            WVASSERTEQ(buf[i], pos++ & 0xFF);
    }
}

static void read_file_in_large_steps() {
    FileRef file(pat_file, FILE_R);

    static uint8_t buf[1024 * 3];
    ssize_t count, pos = 0;
    while((count = file->read(buf, sizeof(buf))) > 0) {
        for(ssize_t i = 0; i < count; ++i)
            WVASSERTEQ(buf[i], pos++ & 0xFF);
    }
}

static void write_file_and_read_again() {
    char content[64] = "Foobar, a test and more and more and more!";
    const size_t contentsz = strlen(content) + 1;

    FileRef file(pat_file, FILE_RW);

    file->write_all(content, contentsz);

    WVASSERTEQ(file->seek(0, M3FS_SEEK_CUR), contentsz);
    WVASSERTEQ(file->seek(0, M3FS_SEEK_SET), 0u);

    char buf[contentsz];
    ssize_t count = file->read(buf, sizeof(buf));
    WVASSERTEQ(count, static_cast<ssize_t>(sizeof(buf)));
    WVASSERTEQ(buf, StringRef(content));

    // undo the write
    file->seek(0, M3FS_SEEK_SET);
    for(size_t i = 0; i < contentsz; ++i)
        content[i] = i;
    file->write(content, contentsz);
}

static void transactions() {
    char content1[] = "Text1";
    char content2[] = "Text2";
    char content3[] = "Text1Text2";
    const char *tmp_file = "/tmp_file.txt";

    {
        FileInfo info;
        FileRef file1(tmp_file, FILE_W | FILE_CREATE);
        file1->write_all(content1, sizeof(content1) - 1);

        {
            FileRef file2(tmp_file, FILE_W | FILE_CREATE);

            WVASSERTERR(Errors::EXISTS, [&file2, &content2] {
                file2->write_all(content2, sizeof(content2) - 1);
            });

            file2->stat(info);
            WVASSERTEQ(info.size, 0u);

            file1->stat(info);
            WVASSERTEQ(info.size, 0u);

            file1->flush();

            file2->stat(info);
            WVASSERTEQ(info.size, sizeof(content1) - 1);

            file1->stat(info);
            WVASSERTEQ(info.size, sizeof(content1) - 1);

            WVASSERTEQ(file2->seek(0, M3FS_SEEK_END), sizeof(content1) - 1);
            file2->write_all(content2, sizeof(content2) - 1);
        }
    }

    {
        FileRef file(tmp_file, FILE_R);

        char buf[sizeof(content3)] = {0};
        WVASSERTEQ(file->read(buf, sizeof(buf)), static_cast<ssize_t>(sizeof(content3) - 1));
        WVASSERTEQ(buf, StringRef(content3));
        WVASSERTEQ(file->read(buf, sizeof(buf)), 0);
    }
}

static void buffered_read_until_end() {
    FStream file(pat_file, FILE_R, 256);

    uint8_t buf[16];
    ssize_t count, pos = 0;
    while((count = file.read(buf, sizeof(buf))) > 0) {
        for(ssize_t i = 0; i < count; ++i)
            WVASSERTEQ(buf[i], pos++ & 0xFF);
    }
    WVASSERT(file.eof() && !file.error());
}

static void buffered_read_with_seek() {
    FStream file(pat_file, FILE_R, 200);

    uint8_t buf[32];
    size_t pos = 0;
    ssize_t count;
    for(int i = 0; i < 10; ++i) {
        count = file.read(buf, sizeof(buf));
        WVASSERTEQ(count, 32);
        for(ssize_t i = 0; i < count; ++i)
            WVASSERTEQ(buf[i], pos++ & 0xFF);
    }

    // we are at pos 320, i.e. we have 200..399 in our buffer
    pos = 220;
    file.seek(pos, M3FS_SEEK_SET);

    count = file.read(buf, sizeof(buf));
    WVASSERTEQ(count, 32);
    for(ssize_t i = 0; i < count; ++i)
        WVASSERTEQ(buf[i], pos++ & 0xFF);

    pos = 405;
    file.seek(pos, M3FS_SEEK_SET);

    while((count = file.read(buf, sizeof(buf))) > 0) {
        for(ssize_t i = 0; i < count; ++i)
            WVASSERTEQ(buf[i], pos++ & 0xFF);
    }
    WVASSERT(file.eof() && !file.error());
}

static void buffered_read_with_large_buf() {
    FStream file(pat_file, FILE_R, 256);

    ssize_t count, pos = 0;
    while((count = file.read(largebuf, sizeof(largebuf))) > 0) {
        for(ssize_t i = 0; i < count; ++i)
            WVASSERTEQ(largebuf[i], pos++ & 0xFF);
    }
    WVASSERT(file.eof() && !file.error());
}

static void buffered_read_and_write() {
    FStream file(pat_file, 600, 256, FILE_RW);

    size_t size = file.seek(0, M3FS_SEEK_END);
    file.seek(0, M3FS_SEEK_SET);

    // overwrite it
    uint8_t val = size - 1;
    for(size_t i = 0; i < size; ++i, --val)
        WVASSERTEQ(file.write(&val, sizeof(val)), static_cast<ssize_t>(sizeof(val)));

    // read it again and check content
    file.seek(0, M3FS_SEEK_SET);
    val = size - 1;
    for(size_t i = 0; i < size; ++i, --val) {
        uint8_t check;
        WVASSERTEQ(file.read(&check, sizeof(check)), static_cast<ssize_t>(sizeof(check)));
        WVASSERTEQ(check, val);
    }

    // restore old content
    file.seek(0, M3FS_SEEK_SET);
    val = 0;
    for(size_t i = 0; i < size; ++i, ++val)
        WVASSERTEQ(file.write(&val, sizeof(val)), static_cast<ssize_t>(sizeof(val)));
    WVASSERT(file.good());
}

static void buffered_write_with_seek() {
    FStream file(pat_file, 600, 256, FILE_RW);

    file.seek(2, M3FS_SEEK_SET);
    file.write("test", 4);

    file.seek(8, M3FS_SEEK_SET);
    file.write("foobar", 6);

    file.seek(11, M3FS_SEEK_SET);
    file.write("foo", 3);

    file.seek(1, M3FS_SEEK_SET);
    char buf[16];
    file.read(buf, 16);
    buf[15] = '\0';
    WVASSERT(file.good());

    char exp[] = {1,'t','e','s','t',6,7,'f','o','o','f','o','o',14,15,0};
    WVASSERTEQ(buf, StringRef(exp));
}

void tfs() {
    RUN_TEST(extending_small_file);
    RUN_TEST(append_bug);
    RUN_TEST(creating_in_steps);
    RUN_TEST(small_write_at_begin);
    RUN_TEST(truncate);
    RUN_TEST(append);
    RUN_TEST(append_with_read);
    RUN_TEST(append_with_commit);
    RUN_TEST(file_mux);
    RUN_TEST(pipe_mux);
    RUN_TEST(file_errors);
    RUN_TEST(read_file_at_once);
    RUN_TEST(read_file_in_64b_steps);
    RUN_TEST(read_file_in_large_steps);
    RUN_TEST(write_file_and_read_again);
    RUN_TEST(transactions);
    RUN_TEST(buffered_read_until_end);
    RUN_TEST(buffered_read_with_seek);
    RUN_TEST(buffered_read_with_large_buf);
    RUN_TEST(buffered_read_and_write);

    // have to be last: overwrite /pat.bin
    RUN_TEST(buffered_write_with_seek);
}
