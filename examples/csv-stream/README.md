# Incremental CSV

`rewind run main.rw` feeds one byte at a time across UTF-8, doubled quotes and CRLF, drains rows and restores the queue with a checkpoint. For larger inputs, feed up to 65,536 bytes at a time and drain rows after each successful feed. A failed feed leaves the reader unchanged; after a queue limit, drain and retry the same input in smaller chunks. `finish` validates EOF; `cancel` discards pending data and closes the reader.
