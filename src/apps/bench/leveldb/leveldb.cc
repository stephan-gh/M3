#include <iostream>
#include <sstream>
#include <string>

#include "leveldb/db.h"
#include "leveldb/write_batch.h"

#define INSERT_COUNT    512
#define STRLEN          1024

using namespace std;

int main(int argc, char** argv)
{
    if(argc < 2) {
        cerr << "Usage: " << argv[0] << " <file>\n";
        return 1;
    }
    // Set up database connection information and open database
    leveldb::DB* db;
    leveldb::Options options;
    options.create_if_missing = true;

    leveldb::Status status = leveldb::DB::Open(options, argv[1], &db);

    if (false == status.ok())
    {
        cerr << "Unable to open/create test database '" << argv[1] << "'" << endl;
        cerr << status.ToString() << endl;
        return -1;
    }

    ostringstream os;
    for(unsigned int i = 0; i < STRLEN; ++i)
        os << "x";

    // Add x values to the database
    std::string value = os.str();
    leveldb::WriteOptions writeOptions;
    for (unsigned int i = 0; i < INSERT_COUNT; ++i)
    {
        ostringstream keyStream;
        keyStream << "Key" << i;

        db->Put(writeOptions, keyStream.str(), value);
    }

    // Iterate over each item in the database
    leveldb::Iterator* it = db->NewIterator(leveldb::ReadOptions());

    for (it->SeekToFirst(); it->Valid(); it->Next())
    {
        //cout << it->key().ToString() << " : " << it->value().ToString() << endl;

        std::string val = it->value().ToString();
    }

    if (false == it->status().ok())
    {
        cerr << "An error was found during the scan" << endl;
        cerr << it->status().ToString() << endl;
    }

    delete it;

    // delete some keys

    std::string keys[] = {"Key1", "Key40", "Key12", "Key16", "_Key77_"};
    for(size_t i = 0; i < sizeof(keys) / sizeof(keys[0]); ++i)
    {
        std::string value;
        leveldb::Status s = db->Get(leveldb::ReadOptions(), keys[i], &value);
        if (s.ok()) {
            s = db->Delete(leveldb::WriteOptions(), keys[i]);
            if (!s.ok())
                cerr << "Unable to delete key " << keys[i] << endl;
        }
        else
            cerr << "Unable to find key " << keys[i] << endl;
    }

    // Close the database
    delete db;
    return 0;
}
