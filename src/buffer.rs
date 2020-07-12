use std::sync::{Condvar, Mutex};

pub struct SpeechLoader {
    stream: deepspeech::dynamic::Stream,
    sample_rate: u32,
    total_samples: usize,
    current_text: String,
    samples_since_change: usize,
    raw_data: Vec<i16>,
}

type DidChange = bool;

impl SpeechLoader {
    pub fn new(stream: deepspeech::dynamic::Stream, sample_rate: u32) -> Self {
        Self {
            stream,
            sample_rate,
            total_samples: 0,
            current_text: String::new(),
            samples_since_change: 0,
            raw_data: Vec::new(),
        }
    }

    pub fn time_since_change(&self) -> std::time::Duration {
        let nanos =
            ((1_000_000_000u64) * (self.samples_since_change as u64)) / (self.sample_rate as u64);
        std::time::Duration::from_nanos(nanos)
    }

    pub fn push(&mut self, data: &[i16]) -> Result<DidChange, deepspeech::errors::DeepspeechError> {
        self.stream.feed_audio(data);
        self.total_samples += data.len();
        let mut next_text = self.stream.intermediate_decode()?;
        if next_text != self.current_text {
            std::mem::swap(&mut self.current_text, &mut next_text);
            self.samples_since_change = 0;
            self.raw_data.extend_from_slice(data);
            Ok(true)
        } else {
            if next_text != "" {
                self.raw_data.extend_from_slice(data);
            }
            self.samples_since_change += data.len();
            Ok(false)
        }
    }

    pub fn current_text(&self) -> &str {
        &self.current_text
    }

    pub fn num_samples(&self) -> usize {
        self.total_samples
    }

    pub fn finish(self) -> (String, Vec<i16>) {
        (self.current_text, self.raw_data)
    }
}

pub struct WaitableBuffer<T: Clone> {
    data: Mutex<Vec<T>>,
    waiter: Condvar,
}

impl<T: Clone> WaitableBuffer<T> {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(Vec::new()),
            waiter: Condvar::new(),
        }
    }

    pub fn push_slice(&self, data: &[T]) {
        let mut lock = self.data.lock().unwrap_or_else(|e| e.into_inner());
        lock.extend_from_slice(data);
        self.waiter.notify_all();
    }

    pub fn wait_until(&self, target: usize) -> Vec<T> {
        let mut lock = self.data.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if lock.len() >= target {
                break;
            }
            lock = self.waiter.wait(lock).unwrap_or_else(|e| e.into_inner());
        }
        let mut retvl = Vec::with_capacity(lock.len());
        retvl.append(&mut lock);
        retvl
    }
}
