//! Async wrapper for [`heapless::mpmc::MpMcQueue`]

mod dequeue;
mod enqueue;

use heapless::mpmc::MpMcQueue as HMpMcQueue;

use crate::{lock::Lock, waker::WakerRegistration};

use self::{dequeue::DequeueFuture, enqueue::EnqueueFuture};

struct WakerStorage<const W: usize> {
    dequeue_wakers: Lock<[WakerRegistration; W]>,
    enqueue_wakers: Lock<[WakerRegistration; W]>,
}

impl<const W: usize> WakerStorage<W> {
    pub const fn new() -> Self {
        Self {
            dequeue_wakers: Lock::new([WakerRegistration::EMPTY; W]),
            enqueue_wakers: Lock::new([WakerRegistration::EMPTY; W]),
        }
    }
}

/// TODO
pub struct MpMcQueue<T, const W: usize, const N: usize>
where
    T: Unpin,
{
    inner: HMpMcQueue<T, N>,
    wakers: WakerStorage<W>,
}

impl<T, const W: usize, const N: usize> MpMcQueue<T, W, N>
where
    T: Unpin,
{
    /// Create a new MpMcQueue
    pub const fn new() -> Self {
        Self {
            inner: HMpMcQueue::new(),
            wakers: WakerStorage::new(),
        }
    }

    /// TODO
    pub fn enqueue<'me>(&'me self, value: T) -> EnqueueFuture<'me, T, W, N> {
        EnqueueFuture::new(self, value)
    }

    /// TODO
    pub fn dequeue<'me>(&'me self) -> DequeueFuture<'me, T, W, N> {
        DequeueFuture::new(self)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use std::println;
    use std::time::Duration;
    use std::vec::Vec;

    use super::MpMcQueue;

    #[tokio::test]
    async fn mpmc() {
        static Q: MpMcQueue<u32, 1, 8> = MpMcQueue::new();

        const MAX: u32 = 100;
        let mut data = Vec::new();
        for i in 0..=MAX {
            data.push(i);
        }

        let dequeuer = |name: &'static str| {
            tokio::task::spawn(async move {
                println!("{}: Dequeueing...", name);
                let mut rx_data = Vec::new();
                for _ in 0..=MAX {
                    let value = Q.dequeue().await;
                    println!("{}: Succesfully dequeued {}", name, value);
                    rx_data.push(value);
                }
                rx_data
            })
        };

        let t1 = dequeuer("T1");
        let t2 = dequeuer("T2");

        let enqueuer = |data: Vec<u32>, name: &'static str| {
            tokio::task::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(1));
                println!("{}: Enqueing...", name);
                for i in data {
                    Q.enqueue(i).await;
                    interval.tick().await;
                    println!("{}: Succesfully enqueued {}", name, i);
                }
            })
        };

        let t3 = enqueuer(data.clone(), "T3");
        let t4 = enqueuer(data, "T4");

        let (t1, t2, t3, t4) = tokio::join!(t1, t2, t3, t4);

        t3.unwrap();
        t4.unwrap();
        let t1 = t1.unwrap();
        let t2 = t2.unwrap();

        for i in 0..=MAX {
            assert_eq!(
                t1.iter().filter(|f| **f == i).count() + t2.iter().filter(|f| **f == i).count(),
                2
            );
        }
    }
}
