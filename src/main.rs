#![feature(type_alias_impl_trait)]

use async_std::{io, net::TcpStream, task};
use futures_util::io::AsyncWriteExt;
use http::Uri;
use hyper::{
    body::HttpBody as _,
    client::connect::{Connected, Connection},
    rt::Executor,
    Body, Client,
};
use pin_project::pin_project;
use std::{
    error::Error,
    future::Future,
    pin::Pin,
    result::Result,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite};
use tokio_compat::io::futures_io::{Compat, FuturesAsyncReadCompatExt};
use tower_service::Service;

struct AsyncStdExecutor;

impl<Fut> Executor<Fut> for AsyncStdExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        task::spawn(async move { fut.await });
    }
}

#[derive(Clone)]
struct AsyncStdTcpConnector;

#[pin_project]
#[derive(Copy, Clone, Debug)]
struct HyperServiceCompat<T> {
    #[pin]
    inner: T,
}

impl<T> From<T> for HyperServiceCompat<T> {
    fn from(inner: T) -> Self {
        Self { inner }
    }
}

impl<T> TokioAsyncRead for HyperServiceCompat<T>
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

impl<T> TokioAsyncWrite for HyperServiceCompat<T>
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

impl<T> Connection for HyperServiceCompat<T> {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

impl Service<Uri> for AsyncStdTcpConnector {
    type Response = HyperServiceCompat<Compat<TcpStream>>;
    type Error = io::Error;
    type Future = impl Future<Output = Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Uri) -> Self::Future {
        Box::pin(async move {
            if req.scheme_str() != Some("http") {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "only http is supported",
                ));
            }

            let port = match req.port() {
                Some(p) => p.as_u16(),
                None => 80,
            };

            let host = match req.host() {
                Some(host) => host,
                _ => return Err(io::Error::new(io::ErrorKind::Other, "missing host in Uri")),
            };

            let stream = TcpStream::connect((host, port)).await?;

            Ok(HyperServiceCompat {
                inner: stream.compat(),
            })
        })
    }
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let client: Client<_, Body> = Client::builder()
        .executor(AsyncStdExecutor)
        .build(AsyncStdTcpConnector);

    let mut res = client.get("http://fsf.org/".parse().unwrap()).await?;

    println!("Response: {}", res.status());
    println!("Headers: {:#?}\n", res.headers());

    while let Some(next) = res.data().await {
        let chunk = next?;
        io::stdout().write_all(&chunk).await?;
    }

    println!("\n\nDone!");

    Ok(())
}
