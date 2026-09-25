use std::sync::Mutex;

/// Byte-capped ring. Oldest bytes are overwritten. Never writes to disk.
pub struct RingBuffer {
    buf: Vec<u8>,
    cap: usize,
    start: usize,
    len: usize,
}

impl RingBuffer {
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            buf: vec![0; cap],
            cap,
            start: 0,
            len: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, mut data: &[u8]) {
        if data.len() > self.cap {
            data = &data[data.len() - self.cap..];
        }
        for &b in data {
            if self.len == self.cap {
                self.start = (self.start + 1) % self.cap;
            } else {
                self.len += 1;
            }
            let tail = (self.start + self.len - 1) % self.cap;
            self.buf[tail] = b;
        }
    }

    /// Chronological recent output for PTY reattach.
    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.start + i) % self.cap]);
        }
        out
    }
}

pub type SharedBuffer = Mutex<RingBuffer>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_newest_bytes() {
        let mut r = RingBuffer::new(4);
        r.push(b"abcd");
        assert_eq!(r.snapshot(), b"abcd");
        r.push(b"ef");
        assert_eq!(r.snapshot(), b"cdef");
        r.push(b"xyz");
        assert_eq!(r.snapshot(), b"fxyz");
    }

    #[test]
    fn truncates_oversize_write() {
        let mut r = RingBuffer::new(3);
        r.push(b"hello");
        assert_eq!(r.snapshot(), b"llo");
    }
}
