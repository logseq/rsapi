use bytes::Bytes;
use futures::Stream;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// Progressed stream of bytes.
pub struct ProgressedBytesStream {
    inner: Vec<u8>,
    offset: usize,
    callback: Box<dyn Fn(usize, usize) + Send + Sync>,
}

impl ProgressedBytesStream {
    pub fn new<F>(inner: Vec<u8>, callback: F) -> Self
    where
        F: Fn(usize, usize) + Send + Sync + 'static,
    {
        Self {
            inner,
            offset: 0,
            callback: Box::new(callback),
        }
    }
}

impl Stream for ProgressedBytesStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.offset >= self.inner.len() {
            return Poll::Ready(None);
        }

        let mut buf = [0; 8 * 1024];
        let mut len = 0;
        // TODO: optimize
        while len < buf.len() && self.offset < self.inner.len() {
            buf[len] = self.inner[self.offset];
            len += 1;
            self.offset += 1;
        }

        (self.callback)(self.offset, self.inner.len());
        Poll::Ready(Some(Ok(Bytes::from(buf[..len].to_vec()))))
    }
}
