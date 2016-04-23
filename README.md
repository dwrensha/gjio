# good job, yo
Asynchronous input and ouput, built on top
of the [gj event loop](https://github.com/dwrensha/gj).

[![Build Status](https://travis-ci.org/dwrensha/gjio.svg?branch=master)](https://travis-ci.org/dwrensha/gjio)
[![Build status](https://ci.appveyor.com/api/projects/status/5xqrvg1dp6cmdbes?svg=true)](https://ci.appveyor.com/project/dwrensha/gjio/branch/master)

Uses `epoll` on Linux and `kqueue` on OSX.
Sorry, `gjio` does not yet support Windows,
although its interface should be a good match for I/O competion ports.
