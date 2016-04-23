// Copyright (c) 2013-2016 Sandstorm Development Group, Inc. and contributors
// Licensed under the MIT License:
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Asynchronous input and output.
//!
//! # Example
//!
//!```
//! extern crate gj;
//! extern crate gjio;
//! use gj::{EventLoop, Promise};
//! use gjio::{AsyncRead, AsyncWrite, Slice};
//!
//! fn echo(mut stream: gjio::SocketStream, buf: Vec<u8>) -> Promise<(), ::std::io::Error> {
//!     stream.try_read(buf, 1).lift().then(move |(buf, n)| {
//!         if n == 0 { // EOF
//!             Promise::ok(())
//!         } else {
//!             stream.write(Slice::new(buf, n)).then(move |slice| {
//!                 echo(stream, slice.buf)
//!             })
//!         }
//!     })
//! }
//!
//! fn main() {
//!     EventLoop::top_level(|wait_scope| -> Result<(), ::std::io::Error> {
//!         let mut event_port = try!(gjio::EventPort::new());
//!         //let (stream1, stream2) = try!(unix::Stream::new_pair());
//!         //let promise1 = echo(stream1, vec![0; 5]); // Tiny buffer just to be difficult.
//!         //let promise2 = stream2.write(b"hello world").lift().then(|(stream, _)| {
//!         //    stream.read(vec![0; 11], 11).map(|(_, buf, _)| {
//!         //        assert_eq!(buf, b"hello world");
//!         //        Ok(())
//!         //    }).lift()
//!         //});
//!         //try!(Promise::all(vec![promise1, promise2].into_iter()).wait(wait_scope));
//!         Ok(())
//!     }).expect("top level");
//! }
//!```


#[macro_use] extern crate gj;
extern crate time;

#[cfg(unix)]
extern crate nix;

use std::cell::{RefCell};
use std::rc::Rc;
use std::collections::BinaryHeap;
use gj::{Promise, PromiseFulfiller};

mod handle_table;
mod sys;

/// A nonblocking input bytestream.
pub trait AsyncRead {
    /// Attempts to read `buf.len()` bytes from the stream, writing them into `buf`.
    /// Returns `self`, the modified `buf`, and the number of bytes actually read.
    /// Returns as soon as `min_bytes` are read or EOF is encountered.
    fn try_read<T>(&mut self, buf: T, min_bytes: usize) -> Promise<(T, usize), ::std::io::Error>
        where T: AsMut<[u8]>;

    /// Like `try_read()`, but returns an error if EOF is encountered before `min_bytes`
    /// can be read.
    fn read<T>(&mut self, buf: T, min_bytes: usize) -> Promise<(T, usize), ::std::io::Error>
        where T: AsMut<[u8]>
    {
        self.try_read(buf, min_bytes).map(move |(buf, n)| {
            if n < min_bytes {
                Err(::std::io::Error::new(::std::io::ErrorKind::Other, "Premature EOF"))
            } else {
                Ok((buf, n))
            }
        })
    }
}

/// A nonblocking output bytestream.
pub trait AsyncWrite {
    /// Attempts to write all `buf.len()` bytes from `buf` into the stream. Returns `self` and `buf`
    /// once all of the bytes have been written.
    fn write<T: AsRef<[u8]>>(&mut self, buf: T) -> Promise<T, ::std::io::Error>;
}

pub struct Slice<T> where T: AsRef<[u8]> {
    pub buf: T,
    pub end: usize,
}

impl <T> Slice<T> where T: AsRef<[u8]> {
    pub fn new(buf: T, end: usize) -> Slice<T> {
        Slice { buf: buf, end: end }
    }
}

impl <T> AsRef<[u8]> for Slice<T> where T: AsRef<[u8]> {
    fn as_ref<'a>(&'a self) -> &'a [u8] {
        &self.buf.as_ref()[0..self.end]
    }
}

#[cfg(unix)]
type RawDescriptor = ::std::os::unix::io::RawFd;

#[cfg(unix)]
type SocketAddressInner = sys::unix::SocketAddressInner;

pub struct EventPort {
    reactor: Rc<RefCell<::sys::Reactor>>,
    timer_inner: Rc<RefCell<TimerInner>>,
}

impl EventPort {
    pub fn new() -> Result<EventPort, ::std::io::Error> {
        Ok( EventPort {
            reactor: Rc::new(RefCell::new(try!(sys::Reactor::new()))),
            timer_inner: Rc::new(RefCell::new(TimerInner::new())),
        })
    }

    pub fn get_network(&self) -> Network {
        Network::new(self.reactor.clone())
    }

    pub fn get_timer(&self) -> Timer {
        Timer::new(self.timer_inner.clone())
    }
}


impl gj::EventPort<::std::io::Error> for EventPort {
    fn wait(&mut self) -> Result<(), ::std::io::Error> {
        let timeout = self.timer_inner.borrow_mut().get_wait_timeout();

        try!(self.reactor.borrow_mut().run_once(timeout));

        self.timer_inner.borrow_mut().update_current_time();
        self.timer_inner.borrow_mut().process();

        Ok(())
    }
}

pub struct Network {
    reactor: Rc<RefCell<::sys::Reactor>>,
}

impl Network {
    fn new(reactor: Rc<RefCell<::sys::Reactor>>) -> Network {
        Network { reactor: reactor }
    }


    pub fn get_tcp_address(&self, addr: ::std::net::SocketAddr) -> SocketAddress {
        SocketAddress::new(SocketAddressInner::new_tcp(self.reactor.clone(), addr))
    }

    #[cfg(unix)]
    pub fn get_unix_address<P: AsRef<::std::path::Path>>(&self, addr: P)
                            -> Result<SocketAddress, ::std::io::Error>
    {
        Ok(SocketAddress::new(try!(SocketAddressInner::new_unix(self.reactor.clone(), addr))))
    }
}

#[cfg(unix)]
type SocketListenerInner = sys::unix::SocketListenerInner;

pub struct SocketAddress {
    inner: SocketAddressInner,
}

impl SocketAddress {
    fn new(inner: SocketAddressInner) -> SocketAddress {
        SocketAddress { inner: inner }
    }

    pub fn connect(&self) -> Promise<SocketStream, ::std::io::Error>
    {
        self.inner.connect().map(|s| Ok(SocketStream::new(s)))
    }

    pub fn listen(&mut self) -> Result<SocketListener, ::std::io::Error>
    {
        Ok(SocketListener::new(try!(self.inner.listen())))
    }
}

pub struct SocketListener {
    inner: Rc<RefCell<SocketListenerInner>>,
}

impl Clone for SocketListener {
    fn clone(&self) -> SocketListener {
        SocketListener { inner: self.inner.clone() }
    }
}

impl SocketListener {
    fn new(inner: SocketListenerInner) -> SocketListener {
        SocketListener { inner: Rc::new(RefCell::new(inner)) }
    }

    pub fn accept(&mut self) -> Promise<SocketStream, ::std::io::Error> {
        let inner = self.inner.clone();
        let inner2 = inner.clone();
        let maybe_queue = inner.borrow_mut().queue.take();
        let promise = match maybe_queue {
            None => SocketListenerInner::accept_internal(inner2),
            Some(queue) => {
                queue.then_else(move |_| SocketListenerInner::accept_internal(inner2) )
            }
        };

        let (p, f) = Promise::and_fulfiller();
        inner.borrow_mut().queue = Some(p);

        promise.map(move |inner| {
            f.resolve(Ok(()));
            Ok(SocketStream::new(inner))
        })
    }
}

#[cfg(unix)]
type SocketStreamInner = sys::unix::SocketStreamInner;

pub struct SocketStream {
    inner: Rc<RefCell<SocketStreamInner>>,
}

impl Clone for SocketStream {
    fn clone(&self) -> SocketStream {
        SocketStream { inner: self.inner.clone() }
    }
}

impl SocketStream {
    fn new(inner: SocketStreamInner) -> SocketStream {
        SocketStream { inner: Rc::new(RefCell::new(inner)) }
    }
}


impl AsyncRead for SocketStream {
    fn try_read<T>(&mut self, buf: T, min_bytes: usize) -> Promise<(T, usize), ::std::io::Error>
        where T: AsMut<[u8]>
    {
        let inner = self.inner.clone();
        let inner2 = inner.clone();
        let maybe_queue = inner.borrow_mut().read_queue.take();
        let promise = match maybe_queue {
            None => SocketStreamInner::try_read_internal(inner2, buf, 0, min_bytes),
            Some(queue) => {
                queue.then_else(move |_| SocketStreamInner::try_read_internal(inner2, buf, 0, min_bytes) )
            }
        };

        let (p, f) = Promise::and_fulfiller();
        inner.borrow_mut().read_queue = Some(p);

        promise.map_else(move |r| {
            f.resolve(Ok(()));
            r
        })
    }
}

impl AsyncWrite for SocketStream {
    fn write<T>(&mut self, buf: T) -> Promise<T, ::std::io::Error>
        where T: AsRef<[u8]>
    {
        let inner = self.inner.clone();
        let inner2 = inner.clone();
        let maybe_queue = inner.borrow_mut().write_queue.take();
        let promise = match maybe_queue {
            None => SocketStreamInner::write_internal(inner2, buf, 0),
            Some(queue) => {
                queue.then_else(move |_| SocketStreamInner::write_internal(inner2, buf, 0) )
            }
        };

        let (p, f) = Promise::and_fulfiller();
        inner.borrow_mut().write_queue = Some(p);

        promise.map_else(move |r| {
            f.resolve(Ok(()));
            r
        })
    }
}

struct AtTimeFulfiller {
    time: ::time::SteadyTime,
    fulfiller: PromiseFulfiller<(), ::std::io::Error>,
}

impl ::std::cmp::PartialEq for AtTimeFulfiller {
    fn eq(&self, other: &AtTimeFulfiller) -> bool {
        self.time == other.time
    }
}

impl ::std::cmp::Eq for AtTimeFulfiller {}

impl ::std::cmp::Ord for AtTimeFulfiller {
    fn cmp(&self, other: &AtTimeFulfiller) -> ::std::cmp::Ordering {
        if self.time > other.time { ::std::cmp::Ordering::Less }
        else if self.time < other.time { ::std::cmp::Ordering::Greater }
        else { ::std::cmp::Ordering::Equal }
    }
}

impl ::std::cmp::PartialOrd for AtTimeFulfiller {
    fn partial_cmp(&self, other: &AtTimeFulfiller) -> Option<::std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}


struct TimerInner {
    heap: BinaryHeap<AtTimeFulfiller>,
    frozen_steady_time: ::time::SteadyTime,
}

impl TimerInner {
    fn new() -> TimerInner {
        TimerInner {
            heap: BinaryHeap::new(),
            frozen_steady_time: ::time::SteadyTime::now(),
        }
    }

    fn update_current_time(&mut self) {
        self.frozen_steady_time = ::time::SteadyTime::now();
    }

    fn get_wait_timeout(&self) -> Option<::time::Duration> {
        match self.heap.peek() {
            None => None,
            Some(ref at_time_fulfiller) => {
                Some(at_time_fulfiller.time - self.frozen_steady_time +
                     ::time::Duration::milliseconds(1))
            }
        }
    }

    fn process(&mut self) {
        loop {
            match self.heap.peek() {
                None => return,
                Some(ref at_time_fulfiller) => {
                    if at_time_fulfiller.time > self.frozen_steady_time {
                        return;
                    }
                }
            }

            match self.heap.pop() {
                None => unreachable!(),
                Some(AtTimeFulfiller { time : _, fulfiller }) => {
                    fulfiller.fulfill(());
                }
            }
        }
    }
}

pub struct Timer {
    inner: Rc<RefCell<TimerInner>>,
}

impl Timer {
    fn new(inner: Rc<RefCell<TimerInner>>) -> Timer {
        Timer { inner: inner }
    }

    pub fn after_delay(&self, delay: ::std::time::Duration) -> Promise<(), ::std::io::Error> {
        let delay = match ::time::Duration::from_std(delay) {
            Ok(d) => d,
            Err(e) => return Promise::err(
                ::std::io::Error::new(::std::io::ErrorKind::Other, format!("{}", e))),
        };
        let time = self.inner.borrow().frozen_steady_time + delay;
        let (p, f) = Promise::and_fulfiller();

        self.inner.borrow_mut().heap.push(AtTimeFulfiller { time: time, fulfiller: f });

        p
    }

   pub fn timeout_after<T>(&self, delay: ::std::time::Duration,
                            promise: Promise<T, ::std::io::Error>) -> Promise<T, ::std::io::Error>
    {
        promise.exclusive_join(self.after_delay(delay).map(|()| {
            Err(::std::io::Error::new(::std::io::ErrorKind::Other, "operation timed out"))
        }))
    }
}
