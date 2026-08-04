# ChloeDB

ChloeDB is a NoSQL DB with key-value store written in Rust. This is only for learning Rust and how database works.

## Features
* **Append-Only Log** - Both insert and remove DB operations will append to the latest file.
* **Binary Log Format** - All values stored in each log will be serialized into bytes and formatted into [crc (to verify if value is not corrupted), timestamp, operation_type, key_len,  val_len, key, value].
* **In-Memory Index** - HashMap value stores the location of the actual value in log files. It contains: { offset (before a value start), val_total_bytes, file_id }.
* **Multi-Segment Log** - When a certain byte threshold exceeded in the latest log, it will create a new log file to append the operation. And the DB load the log files from oldest to newest into HashMap at the start to get the most recent values.

## Operations
* cargo run -- set name example
* cargo run -- get name
* cargo run -- remove name