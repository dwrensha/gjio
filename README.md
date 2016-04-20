# good job, yo
Asynchronous input and ouput, built on top
of the [gj event loop](https://github.com/dwrensha/gj).


Uses `epoll` on Linux and `kqueue` on OSX.
Sorry, `gjio` does not yet support Windows,
although its interface should be a good match for I/O competion ports.
