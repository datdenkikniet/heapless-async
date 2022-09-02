//! An async wrapper around `heapless::spsc::Queue`

use heapless::spsc::Queue as HQueue;

mod producer;
pub use producer::Producer;

mod consumer;
pub use consumer::Consumer;

use crate::{lock::Lock, waker::WakerRegistration};

/// An async queue backed by a heapless [heapless::spsc::Queue]
pub struct Queue<T, const N: usize>
where
    T: Unpin,
{
    inner: HQueue<T, N>,
    producer_waker: Lock<WakerRegistration>,
    consumer_waker: Lock<WakerRegistration>,
}

impl<T, const N: usize> Queue<T, N>
where
    T: Unpin,
{
    /// Create a new Queue
    pub const fn new() -> Self {
        Self {
            inner: HQueue::new(),
            producer_waker: Lock::new(WakerRegistration::new()),
            consumer_waker: Lock::new(WakerRegistration::new()),
        }
    }

    /// Split the queue into a producer and consumer
    pub fn split(&mut self) -> (Producer<'_, T, N>, Consumer<'_, T, N>) {
        let (cw, pw) = (&self.consumer_waker, &self.producer_waker);

        let (producer, consumer) = self.inner.split();
        (
            Producer::new(producer, pw, cw),
            Consumer::new(consumer, pw, cw),
        )
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use std::boxed::Box;
    use std::println;
    use std::time::Duration;
    use std::vec::Vec;

    use super::Queue;

    #[tokio::test]
    async fn spsc() {
        let queue: &'static mut Queue<u32, 8> = Box::leak(Box::new(Queue::new()));

        let (mut tx, mut rx) = queue.split();
        const MAX: u32 = 100;
        let mut data = Vec::new();
        for i in 0..=MAX {
            data.push(i);
        }

        let t1_data = data.clone();
        let t1 = tokio::task::spawn(async move {
            println!("Dequeueing...");
            let mut rx_data = Vec::new();
            loop {
                let value = rx.dequeue().await;
                println!("Succesfully dequeued {}", value);
                rx_data.push(value);
                if value == MAX {
                    break;
                }
            }
            assert_eq!(t1_data, rx_data);
        });

        let t2 = tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(1));
            println!("Enqueing...");
            for i in data {
                tx.enqueue(i).await;
                interval.tick().await;
                println!("Succesfully enqueued {}", i);
            }
        });

        let (t1, t2) = tokio::join!(t1, t2);
        t1.unwrap();
        t2.unwrap();
    }
}
