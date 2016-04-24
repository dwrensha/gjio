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
use std::os::unix::io::RawFd;
use nix::sys::socket;
use handle_table::{Handle};

macro_rules! try_syscall {
    ($expr:expr) => (
        {
            let result;
            loop {
                match $expr {
                    Ok(v) => {
                        result = v;
                        break;
                    }
                    Err(e) if e.errno() == ::nix::Errno::EINTR => continue,
                    Err(e) => return Err(::std::convert::From::from(e))
                }
            }
            result
        }
    )
}

#[cfg(target_os = "macos")]
pub mod kqueue;

#[cfg(target_os = "linux")]
pub mod epoll;

#[cfg(target_os = "macos")]
pub type Reactor = kqueue::Reactor;

#[cfg(target_os = "linux")]
pub type Reactor = epoll::Reactor;

pub struct FdObserver {
    read_fulfiller: Option<PromiseFulfiller<(), ::std::io::Error>>,
    write_fulfiller: Option<PromiseFulfiller<(), ::std::io::Error>>,
}

impl FdObserver {
    pub fn when_becomes_readable(&mut self) -> Promise<(), ::std::io::Error> {
        let (promise, fulfiller) = Promise::and_fulfiller();
        self.read_fulfiller = Some(fulfiller);
        promise
    }

    pub fn when_becomes_writable(&mut self) -> Promise<(), ::std::io::Error> {
        let (promise, fulfiller) = Promise::and_fulfiller();
        self.write_fulfiller = Some(fulfiller);
        promise
    }
}

#[derive(Clone)]
pub struct SocketAddressInner {
    reactor: Rc<RefCell<Reactor>>,
    addr: socket::SockAddr,
}

impl SocketAddressInner {
    pub fn new_tcp(reactor: Rc<RefCell<Reactor>>, addr: ::std::net::SocketAddr) -> SocketAddressInner {
        SocketAddressInner {
            reactor: reactor,
            addr: socket::SockAddr::Inet(socket::InetAddr::from_std(&addr))
        }
    }

    pub fn new_unix<P: AsRef<::std::path::Path>>(reactor: Rc<RefCell<Reactor>>, addr: P)
                                                 -> Result<SocketAddressInner, ::std::io::Error>
    {
        // In what situations does this fail?
        Ok(SocketAddressInner {
            reactor: reactor,
            addr: ::nix::sys::socket::SockAddr::Unix(
                                   try!(::nix::sys::socket::UnixAddr::new(addr.as_ref())))

        })
    }

    pub fn connect(&self) -> Promise<SocketStreamInner, ::std::io::Error> {
        let reactor = self.reactor.clone();
        let addr = self.addr;
        Promise::ok(()).then(move |()| {
            let reactor = reactor;
            let fd = pry!(::nix::sys::socket::socket(addr.family(), ::nix::sys::socket::SockType::Stream,
                                                     ::nix::sys::socket::SOCK_NONBLOCK, 0));
            loop {
                match ::nix::sys::socket::connect(fd, &addr) {
                    Ok(()) => break,
                    Err(e) if e.errno() == ::nix::Errno::EINTR => continue,
                    Err(e) if e.errno() == ::nix::Errno::EINPROGRESS => break,
                    Err(e) => return Promise::err(e.into()),
                }
            }

            let handle = pry!(reactor.borrow_mut().new_observer(fd));

            // TODO: if we're not already connected, maybe only register writable interest,
            // and then reregister with read/write interested once we successfully connect.

            let promise = reactor.borrow_mut().observers[handle].when_becomes_writable();
            promise.map(move |()| {

                let errno = try_syscall!(::nix::sys::socket::getsockopt(
                    fd,
                    ::nix::sys::socket::sockopt::SocketError));
                if errno != 0 {
                    Err(::std::io::Error::from_raw_os_error(errno))
                } else {
                    Ok(SocketStreamInner::new(reactor, handle, fd))
                }
            })
        })
    }

    pub fn listen(&mut self) -> Result<SocketListenerInner, ::std::io::Error>
    {
        let fd = try!(socket::socket(self.addr.family(), socket::SockType::Stream,
                                                 socket::SOCK_NONBLOCK | socket::SOCK_CLOEXEC,
                                                 0));

        try_syscall!(socket::setsockopt(fd, socket::sockopt::ReuseAddr, &true));
        try_syscall!(socket::bind(fd, &self.addr));
        try_syscall!(socket::listen(fd, 1024));


        let handle = try!(self.reactor.borrow_mut().new_observer(fd));
        Ok(SocketListenerInner::new(self.reactor.clone(), handle, fd))
    }
}

pub struct SocketListenerInner {
    reactor: Rc<RefCell<Reactor>>,
    handle: Handle,
    descriptor: RawFd,
    pub queue: Option<Promise<(),()>>,
}

impl Drop for SocketListenerInner {
    fn drop(&mut self) {
        let _ = ::nix::unistd::close(self.descriptor);
        self.reactor.borrow_mut().observers.remove(self.handle);
    }
}

impl SocketListenerInner {
    fn new(reactor: Rc<RefCell<::sys::Reactor>>, handle: Handle, descriptor: RawFd)
           -> SocketListenerInner
    {
        SocketListenerInner {
            reactor: reactor,
            handle: handle,
            descriptor: descriptor,
            queue: None,
        }
    }

    pub fn local_addr(&self) -> Result<::std::net::SocketAddr, ::std::io::Error> {
        match try_syscall!(socket::getsockname(self.descriptor)) {
            socket::SockAddr::Inet(inet_addr) => Ok(inet_addr.to_std()),
            _ => Err(::std::io::Error::new(::std::io::ErrorKind::Other,
                                           "cannot take local_addr of a non-inet socket")),
        }
    }

    pub fn accept_internal(inner: Rc<RefCell<SocketListenerInner>>)
                       -> Promise<SocketStreamInner, ::std::io::Error>
    {
        let fd = inner.borrow_mut().descriptor;
        loop {
            match ::nix::sys::socket::accept4(fd, socket::SOCK_NONBLOCK | socket::SOCK_CLOEXEC) {
                Ok(fd) => {
                    let reactor = inner.borrow().reactor.clone();
                    let handle = pry!(reactor.borrow_mut().new_observer(fd));
                    return Promise::ok(SocketStreamInner::new(reactor, handle, fd));
                }
                Err(e) => {
                    match e.errno() {
                        ::nix::Errno::EINTR => continue,
                        ::nix::Errno::EAGAIN => {
                            let handle = inner.borrow().handle;
                            let promise = {
                                let reactor = &inner.borrow().reactor;
                                let promise = // LOL borrow checker fail.
                                    reactor.borrow_mut().observers[handle].when_becomes_readable();
                                promise
                            };
                            return promise.then(|()| {
                                SocketListenerInner::accept_internal(inner)
                            });
                        }
                        _ => {
                            return Promise::err(e.into());
                        }
                    }
                }
            }
        }
    }

}


pub struct SocketStreamInner {
    reactor: Rc<RefCell<Reactor>>,
    handle: Handle,
    descriptor: RawFd,

    pub read_queue: Option<Promise<(),()>>,
    pub write_queue: Option<Promise<(),()>>,
}

impl Drop for SocketStreamInner {
    fn drop(&mut self) {
        let _ = ::nix::unistd::close(self.descriptor);
        self.reactor.borrow_mut().observers.remove(self.handle);
    }
}

impl SocketStreamInner {
    fn new(reactor: Rc<RefCell<Reactor>>, handle: Handle, descriptor: RawFd) -> SocketStreamInner {
        SocketStreamInner {
                reactor: reactor,
                handle: handle,
                descriptor: descriptor,
                read_queue: None,
                write_queue: None,
        }
    }

    pub fn new_pair(reactor: Rc<RefCell<Reactor>>)
                    -> Result<(SocketStreamInner, SocketStreamInner), ::std::io::Error>
    {
        let (fd0, fd1) = try_syscall!(socket::socketpair(socket::AddressFamily::Unix,
                                                         socket::SockType::Stream,
                                                         0,
                                                         socket::SOCK_NONBLOCK | socket::SOCK_CLOEXEC));
        let handle0 = try!(reactor.borrow_mut().new_observer(fd0));
        let handle1 = try!(reactor.borrow_mut().new_observer(fd1));
        Ok((SocketStreamInner::new(reactor.clone(), handle0, fd0),
            SocketStreamInner::new(reactor, handle1, fd1)))
    }

    pub fn wrap_raw_socket_descriptor(reactor: Rc<RefCell<Reactor>>, fd: RawFd)
                    -> Result<SocketStreamInner, ::std::io::Error>
    {
        try_syscall!(::nix::fcntl::fcntl(fd, ::nix::fcntl::FcntlArg::F_SETFL(::nix::fcntl::O_NONBLOCK)));
        let handle = try!(reactor.borrow_mut().new_observer(fd));

        Ok(SocketStreamInner::new(reactor, handle, fd))
    }

    pub fn socket_spawn<F>(reactor: Rc<RefCell<Reactor>>, start_func: F)
                           -> Result<(::std::thread::JoinHandle<()>, SocketStreamInner), Box<::std::error::Error>>
        where F: FnOnce(::SocketStream, &::gj::WaitScope, ::EventPort) -> Result<(), Box<::std::error::Error>>,
              F: Send + 'static
    {
        use nix::sys::socket::{socketpair, AddressFamily, SockType, SOCK_CLOEXEC, SOCK_NONBLOCK};

        let (fd0, fd1) =
            try_syscall!(socketpair(AddressFamily::Unix, SockType::Stream, 0, SOCK_NONBLOCK | SOCK_CLOEXEC));

        let handle0 = try!(reactor.borrow_mut().new_observer(fd0));

        let join_handle = ::std::thread::spawn(move || {
            let _result = ::gj::EventLoop::top_level(move |wait_scope| {
                let event_port = try!(::EventPort::new());
                let network = event_port.get_network();
                let socket_stream = try!(unsafe { network.wrap_raw_socket_descriptor(fd1) });
                start_func(socket_stream, &wait_scope, event_port)
            });
        });

        Ok((join_handle, SocketStreamInner::new(reactor, handle0, fd0)))
    }

    pub fn try_read_internal<T>(inner: Rc<RefCell<SocketStreamInner>>,
                            mut buf: T,
                            mut already_read: usize,
                            min_bytes: usize)
                            -> Promise<(T, usize), ::std::io::Error>
        where T: AsMut<[u8]>
    {
        while already_read < min_bytes {
            let descriptor = inner.borrow().descriptor;
            match ::nix::unistd::read(descriptor, &mut buf.as_mut()[already_read..]) {
                Ok(0) => {
                    // EOF
                    return Promise::ok((buf, already_read));
                }
                Ok(n) => {
                    already_read += n;
                }
                Err(e) => {
                    match e.errno() {
                        ::nix::Errno::EINTR => continue,
                        ::nix::Errno::EAGAIN => {
                            let handle = inner.borrow().handle;
                            let promise = {
                                let reactor = &inner.borrow().reactor;
                                let promise = // LOL borrow checker fail.
                                    reactor.borrow_mut().observers[handle].when_becomes_readable();
                                promise
                            };

                            return promise.then_else(move |r| match r {
                                Ok(()) => SocketStreamInner::try_read_internal(inner, buf, already_read, min_bytes),
                                Err(e) => Promise::err(e)
                            });
                        }
                        _ => {
                            return Promise::err(e.into())
                        }
                    }
                }
            }
        }
        Promise::ok((buf, already_read))
    }

    pub fn write_internal<T>(inner: Rc<RefCell<SocketStreamInner>>,
                         buf: T,
                         mut already_written: usize) -> Promise<T, ::std::io::Error>
        where T: AsRef<[u8]>
    {
        while already_written < buf.as_ref().len() {
            let descriptor = inner.borrow().descriptor;
            match ::nix::unistd::write(descriptor, &buf.as_ref()[already_written..]) {
                Ok(n) => {
                    already_written += n;
                }
                Err(e) => {
                    match e.errno() {
                        ::nix::Errno::EINTR => continue,
                        ::nix::Errno::EAGAIN => {
                            let handle = inner.borrow().handle;
                            let promise = {
                                let reactor = &inner.borrow().reactor;
                                let promise = // LOL borrow checker fail.
                                    reactor.borrow_mut().observers[handle].when_becomes_writable();
                                promise
                            };
                            return promise.then_else(move |r| match r {
                                Ok(()) => SocketStreamInner::write_internal(inner, buf, already_written),
                                Err(e) => Promise::err(e),
                            })
                        }
                        _ => {
                            return Promise::err(e.into());
                        }
                    }
                }
            }
        }
        Promise::ok(buf)
    }
}

