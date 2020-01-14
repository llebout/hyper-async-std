#![feature(type_alias_impl_trait)]

use async_std::net::TcpStream;
use futures_util::io::AsyncWriteExt;
use http::Uri;
use hyper::body::HttpBody as _;
use hyper::client::connect::{Connected, Connection};
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{self, AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite};
use tokio_compat::io::futures_io::{Compat, FuturesAsyncReadCompatExt};
use tower_service::Service;

struct AsyncStdExecutor;

impl<Fut> hyper::rt::Executor<Fut> for AsyncStdExecutor
where
    Fut: std::future::Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        async_std::task::spawn(async move { fut.await });
    }
}

#[derive(Clone)]
struct AsyncStdTcpConnector;

#[pin_project]
#[derive(Copy, Clone, Debug)]
struct NewCompat<T> {
    #[pin]
    inner: T,
}

impl<T> From<T> for NewCompat<T> {
    fn from(inner: T) -> Self {
        NewCompat { inner }
    }
}

impl<T> TokioAsyncRead for NewCompat<T>
where
    T: TokioAsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_read(cx, buf)
    }
}

impl<T> TokioAsyncWrite for NewCompat<T>
where
    T: TokioAsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().inner.poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().inner.poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().inner.poll_shutdown(cx)
    }
}

impl<T> Connection for NewCompat<T> {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

impl Service<Uri> for AsyncStdTcpConnector {
    type Response = NewCompat<Compat<TcpStream>>;
    type Error = async_std::io::Error;
    type Future = impl Future<Output = std::result::Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Uri) -> Self::Future {
        Box::pin(async move {
            let port = match req.port() {
                Some(p) => p.as_u16(),
                None => 80,
            };

            let host = match req.host() {
                Some(host) => host,
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "missing host in Uri",
                    ))
                }
            };

            let stream = TcpStream::connect((host, port)).await?;

            Ok(NewCompat {
                inner: stream.compat(),
            })
        })
    }
}

#[async_std::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client: hyper::Client<_, hyper::Body> = hyper::Client::builder()
        .executor(AsyncStdExecutor)
        .build(AsyncStdTcpConnector);

    let mut res = client.get("http://fsf.org/".parse().unwrap()).await?;

    println!("Response: {}", res.status());
    println!("Headers: {:#?}\n", res.headers());

    while let Some(next) = res.data().await {
        let chunk = next?;
        async_std::io::stdout().write_all(&chunk).await?;
    }

    println!("\n\nDone!");

    Ok(())
}
