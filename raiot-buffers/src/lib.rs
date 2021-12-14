use std::io::{ErrorKind, Read, Write};

#[derive(Debug)]
/// Represents a piece of the circular buffer, which may be consecutive or split into two slices
pub enum BufferSlice<'a> {
    /// A consecutive slice of the circular buffer
    Consecutive(&'a [u8]),

    /// A non-consecutive slice of the circular buffer
    Splitted(&'a [u8], &'a [u8]),
}

impl Read for BufferSlice<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            BufferSlice::Consecutive(ref mut bytes) => return bytes.read(buf),
            BufferSlice::Splitted(ref mut part1, ref mut part2) => {
                if buf.len() <= part1.len() {
                    part1.read(buf)
                } else {
                    let (part1_buf, part2_buf) = buf.split_at_mut(part1.len());
                    let part1_count = part1.read(part1_buf)?;
                    let part2_count = part2.read(part2_buf)?;
                    let total = part1_count + part2_count;
                    Ok(total)
                }
            }
        }
    }
}

/// A circular buffer of bytes
#[derive(Debug)]
pub struct CircularBuffer {
    buffer: Box<[u8]>,
    read: usize,
    write: usize,
    full: bool,
}

impl CircularBuffer {
    /// Creates a new circular buffer
    ///
    /// # Panics
    /// Panics if the specified buffer size is non-positive
    pub fn new(size: usize) -> CircularBuffer {
        assert!(size > 0, "Circular buffer size must be positive");

        CircularBuffer {
            buffer: vec![0; size].into_boxed_slice(),
            read: 0,
            write: 0,
            full: false,
        }
    }

    /// TRUE if the buffer is completely full
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// TRUE if the buffer is completely empty
    pub fn is_empty(&self) -> bool {
        return (self.read == self.write) && !self.full;
    }

    /// The buffer capacity, in bytes
    pub fn size(&self) -> usize {
        self.buffer.len()
    }

    /// Reads a slice from the buffer without removing it from the buffer
    ///
    /// # Panics
    /// Panics if the buffer contains less data than requested
    pub fn peek(&self, length: usize) -> BufferSlice<'_> {
        if length > self.valid_length() {
            panic!("Not enough valid data!");
        }
        self.get_buffer_slice(self.read, length)
    }

    /// Reads the entire remaining slice from the buffer, wihtout removing it
    pub fn peek_remaining(&self) -> BufferSlice<'_> {
        self.get_buffer_slice(self.read, self.valid_length())
    }

    pub fn write_into<S: Write>(&mut self, writer: &mut S) -> std::io::Result<usize> {
        match self.peek_remaining() {
            BufferSlice::Consecutive(buf) => {
                let size_written = writer.write(buf)?;
                self.read = (self.read + size_written) % self.size();
                self.full = false;
                Ok(size_written)
            }
            BufferSlice::Splitted(buf1, buf2) => {
                let mut size_written = writer.write(buf1)?;
                let mut result = None;
                match writer.write(buf2) {
                    Ok(size) => {
                        size_written += size;
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                    Err(e) if e.kind() == ErrorKind::Interrupted => {}
                    Err(e) => {
                        result = Some(e);
                    }
                }
                self.read = (self.read + size_written) % self.size();
                self.full = false;
                match result {
                    Some(e) => Err(e),
                    None => Ok(size_written),
                }
            }
        }
    }

    /// Reads a slice and removes it from the buffer
    ///
    /// # Panics
    /// Panics if the specified length is zero.
    /// Panics if the buffer contains less data than requested
    pub fn read_bytes(&mut self, length: usize) -> BufferSlice<'_> {
        assert!(length > 0, "Attempted to read zero bytes");
        if length > self.valid_length() {
            panic!("Not enough valid data!");
        }

        let from = self.read;
        self.read = (self.read + length) % self.size();
        self.full = false;
        let slice = self.get_buffer_slice(from, length);
        return slice;
    }

    /// The amount of data bytes currently in the buffer
    pub fn valid_length(&self) -> usize {
        if self.is_full() {
            return self.size();
        } else if self.is_empty() {
            return 0;
        } else if self.write >= self.read {
            self.write - self.read
        } else {
            self.buffer.len() + self.write - self.read
        }
    }

    /// The amount of data bytes that can be written to the buffer.
    /// This is equal to size - valid_length
    pub fn available_space(&self) -> usize {
        if self.is_empty() {
            return self.size();
        } else if self.is_full() {
            return 0;
        }

        let used: usize;
        if self.write >= self.read {
            used = self.write - self.read;
            let available = self.size() - used;
            available
        } else {
            used = self.write + self.size() - self.read;
            let available = self.size() - used;
            available
        }
    }

    /// Writes all the specified bytes into the buffer.
    /// # Errors
    /// If the buffer doesn't have enough free space, a WriteZero error is returned
    pub fn append_all_bytes(&mut self, bytes: &[u8]) -> Result<(), std::io::Error> {
        let available_space = self.available_space();
        if available_space < bytes.len() {
            return Err(std::io::ErrorKind::WriteZero.into());
        }

        let will_be_full = bytes.len() == available_space;

        let end_pos = self.write + bytes.len();
        if end_pos <= self.size() {
            let buffer_slice = &mut self.buffer[self.write..self.write + bytes.len()];
            buffer_slice.copy_from_slice(bytes);
        } else {
            let first_half_size = self.size() - self.write;
            {
                let first_half_buffer_slice =
                    &mut self.buffer[self.write..self.write + first_half_size];
                let first_half_data_slice = &bytes[0..first_half_size];
                first_half_buffer_slice.copy_from_slice(first_half_data_slice);
            }

            let second_half_size = bytes.len() - first_half_size;
            let second_half_buffer_slice = &mut self.buffer[0..second_half_size];
            let second_half_data_slice = &bytes[first_half_size..bytes.len()];
            second_half_buffer_slice.copy_from_slice(second_half_data_slice);
        }

        self.write = (self.write + bytes.len()) % self.size();
        if will_be_full {
            self.full = true;
        }
        Ok(())
    }

    /// Appends bytes from the reader, until either the buffer is full or the reader completes.
    /// On success, returns the amount of data read from the reader.
    ///
    /// # Errors
    /// Any error reading from the reader is returned to the caller
    /// If append_from_reader is called when the buffer is already full, WriteZero error is returned  
    pub fn append_from_reader<R: Read>(&mut self, reader: &mut R) -> Result<usize, std::io::Error> {
        // TODO full indicator

        if self.available_space() == 0 {
            return Err(std::io::ErrorKind::WriteZero.into());
        }

        let mut total_amount_written: usize = 0;

        loop {
            let buf = self.get_next_consecutive_buffer();
            let amount_read = match reader.read(buf) {
                Ok(0) => return Ok(total_amount_written),
                Ok(size) => size,
                Err(e) => return Err(e),
            };

            self.write = (self.write + amount_read) % self.size();
            total_amount_written += amount_read;

            if self.write == self.read {
                self.full = true;
                return Ok(total_amount_written);
            }
        }
    }

    fn get_buffer_slice(&self, from: usize, length: usize) -> BufferSlice<'_> {
        let end_pos = from + length;
        if end_pos <= self.size() {
            let buffer_slice = &self.buffer[from..end_pos];
            BufferSlice::Consecutive(buffer_slice)
        } else {
            let second_half_size = end_pos % self.size();
            let first_half = &self.buffer[from..self.size()];
            let second_half = &self.buffer[0..second_half_size];
            BufferSlice::Splitted(first_half, second_half)
        }
    }

    fn get_next_consecutive_buffer(&mut self) -> &mut [u8] {
        let (from, to) = self.get_next_consecutive_free_space();
        return &mut self.buffer[from..to];
    }

    fn get_next_consecutive_free_space(&mut self) -> (usize, usize) {
        if self.write < self.read {
            (self.write, self.read)
        } else {
            (self.write, self.size())
        }
    }
}

impl Write for CircularBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let write_size = std::cmp::min(self.available_space(), buf.len());
        self.append_all_bytes(&buf[0..write_size])?;
        Ok(write_size)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl BufferSlice<'_> {
        pub(crate) fn len(&self) -> usize {
            match self {
                BufferSlice::Consecutive(x) => x.len(),
                BufferSlice::Splitted(x, y) => x.len() + y.len(),
            }
        }

        pub(crate) fn into_vec(self) -> Vec<u8> {
            match self {
                BufferSlice::Consecutive(x) => x.to_vec(),
                BufferSlice::Splitted(x, y) => {
                    let mut res = Vec::with_capacity(x.len() + y.len());
                    res.extend_from_slice(x);
                    res.extend_from_slice(y);
                    res
                }
            }
        }
    }

    #[test]
    fn test_buffer_write_sanity() {
        let mut sut = CircularBuffer::new(10);
        assert!(sut.is_empty());
        assert!(!sut.is_full());
        assert_eq!(sut.valid_length(), 0);
        assert_eq!(sut.available_space(), 10);
        let test_data = b"0123456789";
        let write_result = sut.write_all(test_data);
        assert!(write_result.is_ok());
        assert!(!sut.is_empty());
        assert!(sut.is_full());
        assert_eq!(sut.valid_length(), 10);
        assert_eq!(sut.available_space(), 0);
    }

    #[test]
    fn test_buffer_consecutives() {
        let mut sut = CircularBuffer::new(10);
        assert_eq!(sut.get_next_consecutive_free_space(), (0, 10));
        let test_data = b"01234";
        sut.write_all(test_data).unwrap();
        assert_eq!(sut.get_next_consecutive_free_space(), (5, 10));
        let read_slice = sut.read_bytes(5);
        assert_eq!(read_slice.len(), 5);
        assert_eq!(sut.get_next_consecutive_free_space(), (5, 10));
        sut.write_all(test_data).unwrap();
        assert_eq!(sut.get_next_consecutive_free_space(), (0, 5));
        let read_slice = sut.read_bytes(5);
        assert_eq!(read_slice.len(), 5);
        sut.write_all(test_data).unwrap();
        assert_eq!(sut.get_next_consecutive_free_space(), (5, 10));
    }

    #[test]
    fn test_buffer_append_from_reader_available_space() {
        let mut sut = CircularBuffer::new(10);
        assert_eq!(sut.available_space(), 10);
        let mut test_data: Vec<u8> = Vec::new();
        for i in 0u8..5 {
            test_data.push(i);
        }

        let mut appended_size = sut.append_from_reader(&mut &test_data[..]).unwrap();
        assert_eq!(appended_size, 5);
        assert_eq!(sut.available_space(), 5);
        appended_size = sut.append_from_reader(&mut &test_data[..]).unwrap();
        assert_eq!(appended_size, 5);
        assert_eq!(sut.available_space(), 0);
        let read_slice = sut.read_bytes(5);
        assert_eq!(read_slice.len(), 5);
        assert_eq!(sut.available_space(), 5);
        let read_slice = sut.read_bytes(5);
        assert_eq!(read_slice.len(), 5);
        assert_eq!(sut.available_space(), 10);
    }

    #[test]
    fn test_buffer_write_available_space() {
        let mut sut = CircularBuffer::new(10);
        assert_eq!(sut.available_space(), 10);
        assert_eq!(sut.is_empty(), true);
        let test_data = b"01234";
        sut.write_all(test_data).unwrap();
        assert_eq!(sut.available_space(), 5);
        sut.write_all(test_data).unwrap();
        assert_eq!(sut.available_space(), 0);
        assert_eq!(sut.is_full(), true);
        let read_slice = sut.read_bytes(5);
        assert_eq!(read_slice.len(), 5);
        assert_eq!(sut.available_space(), 5);
        let read_slice = sut.read_bytes(5);
        assert_eq!(read_slice.len(), 5);
        assert_eq!(sut.available_space(), 10);
        assert_eq!(sut.is_empty(), true);
    }

    #[test]
    fn test_buffer_circular_write() {
        let mut sut = CircularBuffer::new(15);
        assert_eq!(sut.valid_length(), 0);
        assert_eq!(sut.available_space(), 15);
        let test_data = b"0123456789";
        sut.append_all_bytes(test_data).unwrap();
        assert_eq!(sut.valid_length(), 10);
        assert_eq!(sut.available_space(), 5);
        {
            let read_result = sut.read_bytes(10);
            assert_eq!(test_data, &read_result.into_vec()[..]);
        }
        assert_eq!(sut.valid_length(), 0);
        assert_eq!(sut.available_space(), 15);
        {
            sut.append_all_bytes(test_data).unwrap();
            let read_result = sut.read_bytes(10);
            assert_eq!(test_data, &read_result.into_vec()[..]);
        }
        assert_eq!(sut.valid_length(), 0);
        assert_eq!(sut.available_space(), 15);
    }
}
