// Reference: https://blog.cloudflare.com/pin-and-unpin-in-rust/

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() {
    let (resp, time) = TimedWrapper::new(reqwest::get("https://google.com/ncr")).await;
    println!(
        "Got a HTTP {} in {}ms",
        resp.unwrap().status(),
        time.as_millis()
    );
}

#[pin_project::pin_project]
pub struct TimedWrapper<Fut: Future> {
    start: Option<Instant>,

    #[pin]
    future: Fut,
}

impl<Fut: Future> TimedWrapper<Fut> {
    pub fn new(future: Fut) -> Self {
        Self {
            start: None,
            future,
        }
    }
}

impl<Fut: Future> Future for TimedWrapper<Fut> {
    type Output = (Fut::Output, Duration);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let start = this.start.get_or_insert_with(Instant::now);
        let inner_poll = this.future.poll(cx);
        let elapsed = start.elapsed();

        match inner_poll {
            Poll::Pending => Poll::Pending,
            Poll::Ready(output) => Poll::Ready((output, elapsed)),
        }
    }
}
