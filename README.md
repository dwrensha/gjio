# good job I/O

WARNING:
as of the [0.8 relase of capnp-rpc-rust](https://dwrensha.github.io/capnproto-rust/2017/01/04/rpc-futures.html),
this library is DEPRECATED. Please use [tokio-core](https://github.com/tokio-rs/tokio-core) instead.

Asynchronous input and ouput, built on top
of the [gj event loop](https://github.com/dwrensha/gj).

[![crates.io](http://meritbadge.herokuapp.com/gjio)](https://crates.io/crates/gjio)
[![Build Status](https://travis-ci.org/dwrensha/gjio.svg?branch=master)](https://travis-ci.org/dwrensha/gjio)
[![Build status](https://ci.appveyor.com/api/projects/status/5xqrvg1dp6cmdbes?svg=true)](https://ci.appveyor.com/project/dwrensha/gjio/branch/master)

[documentation](https://docs.rs/gjio/)

Uses `epoll` on Linux, `kqueue` on OSX, and I/O completion ports on Windows.

