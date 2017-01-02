# good job, yo
Asynchronous input and ouput, built on top
of the [gj event loop](https://github.com/dwrensha/gj).

[![crates.io](http://meritbadge.herokuapp.com/gjio)](https://crates.io/crates/gjio)
[![Build Status](https://travis-ci.org/dwrensha/gjio.svg?branch=master)](https://travis-ci.org/dwrensha/gjio)
[![Build status](https://ci.appveyor.com/api/projects/status/5xqrvg1dp6cmdbes?svg=true)](https://ci.appveyor.com/project/dwrensha/gjio/branch/master)

[documentation](https://docs.rs/gjio/0.1.3/gjio/)

Uses `epoll` on Linux, `kqueue` on OSX, and I/O completion ports on Windows.

