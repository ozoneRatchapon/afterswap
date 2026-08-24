//! Rolling price-window store for tournament replay.

use std::collections::VecDeque;

/// Ring buffer of ticks, sliced into overlapping evaluation windows.
pub struct WindowStore {
    prices: VecDeque<f64>,
    capacity: usize,
    window_len: usize,
    stride: usize,
    max_windows: usize,
    /// Total ticks ever pushed (global tick index of the NEXT push).
    total_ticks: u64,
}

impl WindowStore {
    /// Store sized to hold `max_windows` strided windows.
    pub fn new(window_len: usize, stride: usize, max_windows: usize) -> Self {
        let capacity = window_len + stride * max_windows.saturating_sub(1);
        Self {
            prices: VecDeque::with_capacity(capacity + 1),
            capacity,
            window_len,
            stride,
            max_windows,
            total_ticks: 0,
        }
    }

    /// Push one tick; evicts the oldest when full.
    pub fn push(&mut self, price: f64) {
        if self.prices.len() >= self.capacity {
            self.prices.pop_front();
        }
        self.prices.push_back(price);
        self.total_ticks += 1;
    }

    /// Global tick index of the most recent push (0-based), or None when empty.
    pub fn last_tick(&self) -> Option<u64> {
        match self.total_ticks {
            0 => None,
            t => Some(t - 1),
        }
    }

    /// Number of buffered ticks.
    pub fn len(&self) -> usize {
        self.prices.len()
    }

    /// True when no ticks are buffered.
    pub fn is_empty(&self) -> bool {
        self.prices.is_empty()
    }

    /// Latest price, if any.
    pub fn last_price(&self) -> Option<f64> {
        self.prices.back().copied()
    }

    /// Extract up to `max_windows` most-recent strided windows (oldest first).
    /// Each window is `window_len` contiguous ticks.
    pub fn windows(&self) -> Vec<Vec<f64>> {
        let n = self.prices.len();
        if n < self.window_len {
            return Vec::new();
        }
        let buf: Vec<f64> = self.prices.iter().copied().collect();
        let mut out = Vec::new();
        // Walk window starts back from the newest full window.
        let newest_start = n - self.window_len;
        let mut starts = Vec::new();
        let mut s = newest_start;
        loop {
            starts.push(s);
            if s < self.stride || starts.len() >= self.max_windows {
                break;
            }
            s -= self.stride;
        }
        starts.reverse();
        for st in starts {
            out.push(buf[st..st + self.window_len].to_vec());
        }
        out
    }
}

impl WindowStore {
    /// Up to `n` most-recent prices, oldest first.
    pub fn recent(&self, n: usize) -> Vec<f64> {
        let len = self.prices.len();
        let skip = len.saturating_sub(n);
        self.prices.iter().skip(skip).copied().collect()
    }
}
