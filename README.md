# hyper-async-std

Attempt at using hyper with the async-std runtime and **only the async-std runtime**. tokio is included only to construct the AsyncRead and AsyncWrite traits compat layer.

This is currently accomplished using the hyper's executor agnostic traits and tokio-compat with this PR:

https://github.com/leo-lb/tokio-compat/pull/1

The PR adds support for converting between `tokio`'s AsyncRead/AsyncWrite traits and `futures`'s AsyncRead/AsyncWrite traits.

Requires Rust nightly because of `#![feature(type_alias_impl_trait)]` for better ergonomics.

**Only the HTTP client of hyper is currently supported**, implementing HTTP server support has yet to be implemented and **help would be appreciated on this**.
