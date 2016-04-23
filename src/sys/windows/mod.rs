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

use gj::{Promise, PromiseFulfiller};
use std::cell::{RefCell};
use std::rc::Rc;

pub struct Reactor {
    cp: ::miow::iocp::CompletionPort,
}

impl Reactor {
    pub fn new() -> Result<Reactor, ::std::io::Error> {
        Reactor {
            cp: try!(::miow::iocp::CompletionPort::new(1)),
        }
    }

    pub fn run_once(&mut self, maybe_timeout: Option<::time::Duration>)
                    -> Result<(), ::std::io::Error>
    {
        unimplemented!()
    }
}

#[derive(Clone)]
pub struct SocketAddressInner {
    reactor: Rc<RefCell<Reactor>>,
    addr: ::std::net::SocketAddr,
}

impl SocketAddressInner {
    pub fn new_tcp(reactor: Rc<RefCell<Reactor>>, addr: ::std::net::SockAddr)
                   -> SocketAddressInner
    {
        SocketAddresInner {
            reactor: reactor,
            addr: addr,
        }
    }

    pub fn connect(&self) -> Promise<SocketStreamInner, ::std::io::Error> {
        unimplemented!()
    }

    pub fn listen(&mut self) -> Result<SocketListenerInner, ::std::io::Error> {
        unimplemented!()
    }
}

pub struct SocketListenerInner {
    reactor: Rc<RefCell<Reactor>>,
}

impl SocketListenerInner {
    pub fn accept_internal(inner: Rc<RefCell<SocketListenerInner>>)
                           -> Promise<SocketStreamInner, ::std::io::Error>
    {
        unimplemented!()
    }
}

pub struct SocketStreamInner {
    reactor: Rc<RefCell<Reactor>>,
}

impl SocketStreamInner {
    pub fn try_read_internal<T>(inner: Rc<RefCell<SocketStreamInner>>,
                                mut buf: T,
                                mut already_read: usize,
                                min_bytes: usize)
                                ->Promise<(T, usize), ::std::io::Error>
        where T: AsMut<[u8]>
    {
        unimplemented!()
    }

    pub fn write_internal<T>(inner: Rc<RefCell<SocketStreamInner>>,
                             buf: T,
                             mut already_written: usize) -> Promise<T, ::std::io::Error>
        where T: AsRef<[u8]>
    {
        unimplemented!()
    }
}
