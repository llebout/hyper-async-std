# hyper-async-std

Attempt at using hyper with the async-std runtime.

This is currently accomplished using the hyper's executor agnostic traits and tokio-compat with this PR:

https://github.com/leo-lb/tokio-compat/pull/1

The PR adds support for converting between `tokio`'s AsyncRead/AsyncWrite traits and `futures`'s AsyncRead/AsyncWrite traits.
