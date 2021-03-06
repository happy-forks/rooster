// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Type-safe bindings for Magenta fifo objects.

use {HandleBase, Handle, HandleRef, Status};
use {sys, into_result};

/// An object representing a Magenta fifo.
///
/// As essentially a subtype of `Handle`, it can be freely interconverted.
#[derive(Debug, Eq, PartialEq)]
pub struct Fifo(Handle);

impl HandleBase for Fifo {
    fn get_ref(&self) -> HandleRef {
        self.0.get_ref()
    }

    fn from_handle(handle: Handle) -> Self {
        Fifo(handle)
    }
}

impl Fifo {
    /// Create a pair of fifos and return their endpoints. Writing to one endpoint enqueues an
    /// element into the fifo from which the opposing endpoint reads. Wraps the
    /// [mx_fifo_create](https://fuchsia.googlesource.com/magenta/+/master/docs/syscalls/fifo_create.md)
    /// syscall.
    pub fn create(elem_count: u32, elem_size: u32, options: FifoOpts)
        -> Result<(Fifo, Fifo), Status>
    {
        let mut out0 = 0;
        let mut out1 = 0;
        let status = unsafe {
            sys::mx_fifo_create(elem_count, elem_size, options as u32, &mut out0, &mut out1)
        };
        into_result(status, || (Self::from_handle(Handle(out0)), Self::from_handle(Handle(out1))))
    }

    /// Attempts to write some number of elements into the fifo. The number of bytes written will be
    /// rounded down to a multiple of the fifo's element size.
    /// Return value (on success) is number of elements actually written.
    ///
    /// Wraps
    /// [mx_fifo_write](https://fuchsia.googlesource.com/magenta/+/master/docs/syscalls/fifo_write.md).
    pub fn write(&self, bytes: &[u8]) -> Result<u32, Status> {
        let mut num_entries_written = 0;
        let status = unsafe {
            sys::mx_fifo_write(self.raw_handle(), bytes.as_ptr(), bytes.len(),
                &mut num_entries_written)
        };
        into_result(status, || num_entries_written)
    }

    /// Attempts to read some number of elements out of the fifo. The number of bytes read will
    /// always be a multiple of the fifo's element size.
    /// Return value (on success) is number of elements actually read.
    ///
    /// Wraps
    /// [mx_fifo_read](https://fuchsia.googlesource.com/magenta/+/master/docs/syscalls/fifo_read.md).
    pub fn read(&self, bytes: &mut [u8]) -> Result<u32, Status> {
        let mut num_entries_read = 0;
        let status = unsafe {
            sys::mx_fifo_read(self.raw_handle(), bytes.as_mut_ptr(), bytes.len(),
                &mut num_entries_read)
        };
        into_result(status, || num_entries_read)
    }
}

/// Options for creating a fifo pair.
#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FifoOpts {
    /// Default options.
    Default = 0,
}

impl Default for FifoOpts {
    fn default() -> Self {
        FifoOpts::Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_basic() {
        let (fifo1, fifo2) = Fifo::create(4, 2, FifoOpts::Default).unwrap();

        // Trying to write less than one element should fail.
        assert_eq!(fifo1.write(b""), Err(Status::ErrOutOfRange));
        assert_eq!(fifo1.write(b"h"), Err(Status::ErrOutOfRange));

        // Should write one element "he" and ignore the last half-element as it rounds down.
        assert_eq!(fifo1.write(b"hex").unwrap(), 1);

        // Should write three elements "ll" "o " "wo" and drop the rest as it is full.
        assert_eq!(fifo1.write(b"llo worlds").unwrap(), 3);

        // Now that the fifo is full any further attempts to write should fail.
        assert_eq!(fifo1.write(b"blah blah"), Err(Status::ErrShouldWait));

        // Read all 4 entries from the other end.
        let mut read_vec = vec![0; 8];
        assert_eq!(fifo2.read(&mut read_vec).unwrap(), 4);
        assert_eq!(read_vec, b"hello wo");

        // Reading again should fail as the fifo is empty.
        assert_eq!(fifo2.read(&mut read_vec), Err(Status::ErrShouldWait));
    }
}
