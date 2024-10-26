# 0. Write Ahead Log

Status: draft

## Overview

When a client disconnects and reconnects *sometime* in the future, it may miss messages that were published while it was disconnected.

Disconnects can happen for a variety of reasons including:
- Client network issues
- Servier network issues
- Coding errors

A write ahead log would provide a **memory** for the system, guaranteeing that all messages received by the server are delivered to each subscribed client. When a client reconnects, it starts processing messages from the point at which it left off.

Beyond this, a write ahead log allows better decoupling of the server's message receiving and message sending tasks. Instead of iterating over a message's subscribers to send the messages, the receive task simply writes an entry to the WAL and sender tasks subsequently read that.

With each connection processed independently, one hevily subscribed client

## Tradeoffs

A write ahead log is likely to increase the latency between a publish and a receive, compared with a system that fans out to each subscriber.

Latency and throughput become less intertwined.

## Implementation

There are two new classes, a `LogWriter` and a `LogReader`.

The `LogWriter` owns a shared reference to a writeable file stream. The `LogReader` owns a a readable file stream.

Log files are stored in a directory, named using the timestamp of the first message received. A configurable max log file size controls when the log file is rotated. The files contain binary msgpack packed messages.

When a new `LogReader` is created, it can specify a timestamp at which to start processing messages from. The filenames are used to find the first file to process, and then messages within that file are parsed until the specified timestamp has been reached.



## Pseudo code

```
// Pseudo code

class MessageLogWriter {
    buf // Message buffer
    root_dir // path to root directory of log files
    cursor_path // path to current log file
    cursor_size // size of current log file

    pub fn write(message) {
        // Write message to buffer
        self.buf.write(message.to_bytes())
        // Check size of buffer, flush if full
        If buffer > BUFFER_SIZE {
            self.flush_buffer()
        }
    }

    fn flush_buffer() {
        // Open current log file
        stream = open(self.cursor_path)
        // Seek to end
        stream.seek(self.cursor_size)
        // Write contents of buffer
        stream.write(self.buf)
        // Close file
        stream.close()

        // Update cursor size with buf length
        self.cursor_size += self.buf.len()
        
        // Check if log file needs rotating
        if self.cursor_size > LOG_FILE_SIZE {
            self.rotate_log_file()
        }

        // Clear buffer
        self.buf = Bytes::new();
    }

    fn rotate_log_file() {
        // Generate a new file path
        self.cursor_path = self.generate_log_path()
        self.cursor_size = 0
    }

    fn generate_log_path() {
        timestamp = NOW().isoformat()
        return "{root_dir}/{timestamp}.dat"
    }
}

class MessageLogReader {
    buf // memory buffer
    cursor_path // path to current log file
    stream // open stream to read messages from
    fn recv_as_of(as_of: timestamp) -> Message {
        // Find log file containing timestamp
        cursor_path = self.file_contains(timestamp)
        cursor_offset = self.scroll_to_timestamp(timestamp)
    }
    fn recv() {
        // check if message can be read from buffer
        message = self.parse(buf)
        if message is None {
            // fill buffer
            self.fill_buffer()
            // parse message
            message = self.parse(buf)

            if message is None {
                // end of stream, return None or Error
                return None
            }
        }
        return message
    }

    /// Return path of log file that contains the specified timestamp
    fn file_contains(target_timestamp) {
        // List contents of root dir
        files = self.root_dir.files()
        // Iterate until file found
        for file in files {
            file_timestamp = datetime.fromisoformat()
            if file_timestamp < target_timestamp {
                return file
            } 
        }
        return None
    }

    fn scroll_to_timestamp(target_timestamp) {
        cursor // offset
        // parse messages until timestamp is reached
        loop {
            // Read next message without advancing cursor (?)
            message, bytes_read = self.parse(self.stream.peek());
            if message.timestamp > target_timestamp {
                // reached target timestamp
                return
            } else {
                // Advance cursor
                self.stream.seek(bytes_read)
            }
        }
    }

    fn fill_buffer() {
        loop {
            // Try writing from current stream
            let bytes_written = self.stream.write_to_buffer(self.buf)
            if bytes_written == 0 {
                // Current stream exhausted, check if newer path
                next_cursor_path = self.next_cursor_path()
                if next_cursor_path.is_some() {
                    // Advance to next path
                    self.cursor_path = next_cursor_path
                    self.stream = self.cursor_path.open()
                } else {
                    // At end of stream, wait for more messages
                    continue
                }
                // advance cursor to the next path
                self.cursor_path = self.next_cursor_path()
                
            }
        }
    }
}
```
