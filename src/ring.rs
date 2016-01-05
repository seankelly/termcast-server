
pub struct RingBuffer {
    index: usize,
    size: usize,
    buffer: Vec<u8>,
}

pub struct Iter<'a> {
    index: usize,
    offset: usize,
    size: usize,
    buffer: &'a Vec<u8>,
}

impl RingBuffer {
    pub fn new(size: usize) -> Self {
        assert!(size > 0);
        RingBuffer {
            index: 0,
            size: size,
            buffer: Vec::with_capacity(size),
        }
    }

    pub fn add(&mut self, buffer: &[u8]) {
        for byte in buffer {
            if self.buffer.len() < self.size {
                self.buffer.push(*byte);
            }
            else {
                self.buffer[self.index] = *byte;
            }
            self.index += 1;

            if self.index >= self.size {
                self.index = 0;
            }
        }
    }

    // Adds data to the ring buffer but does not wrap around. This is for the specific case of
    // buffering data at the beginning.
    pub fn add_no_wraparound(&mut self, buffer: &[u8]) -> Result<(), ()> {
        for byte in buffer {
            if self.buffer.len() < self.size {
                self.buffer.push(*byte);
            }
            else if self.index >= self.size {
                return Err(());
            }
            self.index += 1;
        }

        Ok(())
    }

    pub fn iter(&self) -> Iter {
        // If the buffer length is less than the size then have not wrapped around yet so no offset
        // is necessary. If it has wrapped then need to start at the next byte to overwrite.
        let offset = self.get_offset();
        Iter {
            index: 0,
            offset: offset,
            size: self.buffer.len(),
            buffer: &self.buffer,
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.index = 0;
    }

    fn get_offset(&self) -> usize {
        if self.buffer.len() < self.size { 0 } else { self.index }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.index < self.size {
            let index = (self.index + self.offset) % self.size;
            /*
            let index = if self.index + self.offset < self.size {
                self.index + self.offset
            }
            else {
                self.index + self.offset - self.size
            };
            */
            self.index += 1;
            Some(self.buffer[index])
        }
        else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;

    #[test]
    fn add() {
        let mut ring = RingBuffer::new(4);
        let bytes = &[0, 1, 2, 3, 4, 5, 6];

        ring.add(&bytes[0..1]);
        assert_eq!(ring.len(), 1);
        let buf: Vec<u8> = ring.iter().collect();
        assert_eq!(buf, vec![0]);

        ring.add(&bytes[1..3]);
        assert_eq!(ring.len(), 3);
        let buf: Vec<u8> = ring.iter().collect();
        assert_eq!(buf, vec![0, 1, 2]);

        ring.add(&bytes[3..4]);
        assert_eq!(ring.len(), 4);
        let buf: Vec<u8> = ring.iter().collect();
        assert_eq!(buf, vec![0, 1, 2, 3]);

        ring.add(&bytes[4..6]);
        assert_eq!(ring.len(), 4);
        let buf: Vec<u8> = ring.iter().collect();
        assert_eq!(buf, vec![2, 3, 4, 5]);

        ring.add(bytes);
        assert_eq!(ring.len(), 4);
        let buf: Vec<u8> = ring.iter().collect();
        assert_eq!(buf, vec![3, 4, 5, 6]);
    }

    #[test]
    fn add_no_wraparound() {
        let mut ring = RingBuffer::new(4);
        let bytes = &[0, 1, 2, 3, 4];

        assert!(ring.add_no_wraparound(&bytes[0..1]).is_ok(), "Append first byte.");
        assert_eq!(ring.len(), 1);

        assert!(ring.add_no_wraparound(&bytes[1..3]).is_ok(), "Append second and third bytes.");
        assert_eq!(ring.len(), 3);

        assert!(ring.add_no_wraparound(&bytes[3..4]).is_ok(), "Append fourth byte.");
        assert_eq!(ring.len(), 4);

        assert!(ring.add_no_wraparound(&bytes[4..5]).is_err(), "Failed to add fifth byte.");
        assert_eq!(ring.len(), 4);
    }

    #[test]
    fn clear() {
        let mut ring = RingBuffer::new(4);
        let bytes = &[0, 1, 2, 3, 4, 5, 6];
        ring.add(bytes);
        assert_eq!(ring.len(), 4);
        ring.clear();
        assert_eq!(ring.len(), 0);

        ring.add(&bytes[0..3]);
        assert_eq!(ring.len(), 3);
        ring.clear();
        assert_eq!(ring.len(), 0);
    }
}
