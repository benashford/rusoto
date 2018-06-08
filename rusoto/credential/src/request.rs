use std::io::Error as IoError;
use std::io::ErrorKind::InvalidData;
use std::mem;
use std::time::{Duration, Instant};

use futures::stream::Concat2;
use futures::{Async, Future, Poll, Stream};
use hyper::client::{HttpConnector, ResponseFuture as HyperFutureResponse};
use hyper::{Body, Client as HyperClient, Request};
use tokio::timer::Deadline;

use super::CredentialsError;

/// A future that will resolve to an `HttpResponse`.
pub struct HttpClientFuture(ClientFutureInner);

enum RequestFuture {
    Waiting(HyperFutureResponse),
    Buffering(Concat2<Body>),
    Swapping,
}

impl Future for RequestFuture {
    type Item = String;
    type Error = CredentialsError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match mem::replace(self, RequestFuture::Swapping) {
            RequestFuture::Waiting(mut hyper_future) => match hyper_future.poll()? {
                Async::NotReady => {
                    *self = RequestFuture::Waiting(hyper_future);
                    Ok(Async::NotReady)
                }
                Async::Ready(res) => {
                    if !res.status().is_success() {
                        Err(CredentialsError {
                            message: format!("Invalid Response Code: {}", res.status()),
                        })
                    } else {
                        *self = RequestFuture::Buffering(res.into_body().concat2());
                        self.poll()
                    }
                }
            },
            RequestFuture::Buffering(mut concat_future) => match concat_future.poll()? {
                Async::NotReady => {
                    *self = RequestFuture::Buffering(concat_future);
                    Ok(Async::NotReady)
                }
                Async::Ready(body) => {
                    let string_body = String::from_utf8(body.to_vec())
                        .map_err(|_| IoError::new(InvalidData, "Non UTF-8 Data returned"))?;
                    Ok(Async::Ready(string_body))
                }
            },
            RequestFuture::Swapping => unreachable!(),
        }
    }
}

enum ClientFutureInner {
    Request(Deadline<RequestFuture>),
    Error(String),
}

impl Future for HttpClientFuture {
    type Item = String;
    type Error = CredentialsError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.0 {
            ClientFutureInner::Error(ref message) => Err(CredentialsError {
                message: message.clone(),
            }),
            ClientFutureInner::Request(ref mut select_future) => match select_future.poll() {
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Ok(Async::Ready(body)) => Ok(Async::Ready(body)),
                Err(de) => match de.into_inner() {
                    Some(e) => Err(e),
                    None => Err(CredentialsError {
                        message: "Request timed out".into(),
                    }),
                },
            },
        }
    }
}

/// Http client for use in a credentials provider.
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: HyperClient<HttpConnector>,
}

impl HttpClient {
    /// Create an http client.
    pub fn new() -> HttpClient {
        HttpClient {
            inner: HyperClient::new(),
        }
    }

    pub fn request(&self, request: Request<Body>, timeout: Duration) -> HttpClientFuture {
        let request_future = RequestFuture::Waiting(self.inner.request(request));
        let timeout = Instant::now() + timeout;
        let deadline = Deadline::new(request_future, timeout);

        HttpClientFuture(ClientFutureInner::Request(deadline))
    }
}
