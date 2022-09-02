/// Copyright (C) 2016 whitequark@whitequark.org
///
/// Permission to use, copy, modify, and/or distribute this software for
/// any purpose with or without fee is hereby granted.
///
/// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
/// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
/// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
/// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
/// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN
/// AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT
/// OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
///
/// A more lightweight waker type. Taken from [smoltcp]
///
/// [smoltcp]: https://github.com/smoltcp-rs/smoltcp/blob/master/LICENSE-0BSD.txt
use core::task::Waker;

/// Utility struct to register and wake a waker.
#[derive(Debug)]
pub struct WakerRegistration {
    waker: Option<Waker>,
}

impl WakerRegistration {
    pub const fn new() -> Self {
        Self { waker: None }
    }

    /// Register a waker. Overwrites the previous waker, if any.
    pub fn register(&mut self, w: &Waker) {
        match self.waker {
            // Optimization: If both the old and new Wakers wake the same task, we can simply
            // keep the old waker, skipping the clone. (In most executor implementations,
            // cloning a waker is somewhat expensive, comparable to cloning an Arc).
            Some(ref w2) if (w2.will_wake(w)) => {}
            // In all other cases
            // - we have no waker registered
            // - we have a waker registered but it's for a different task.
            // then clone the new waker and store it
            _ => self.waker = Some(w.clone()),
        }
    }

    /// Wake the registered waker, if any.
    pub fn wake(&mut self) {
        self.waker.take().map(|w| w.wake());
    }
}
