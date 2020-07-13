use crate::error::{AssistantRsError, CpalError};
use cpal::traits::{DeviceTrait, StreamTrait};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

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

    pub fn time_since_change(&self) -> Duration {
        let nanos =
            ((1_000_000_000u64) * (self.samples_since_change as u64)) / (self.sample_rate as u64);
        Duration::from_nanos(nanos)
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

    pub fn wait_until_timeout(&self, target: usize, timeout: Duration) -> Result<Vec<T>, Timeout> {
        let mut lock = self.data.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if lock.len() >= target {
                break;
            }
            let (l, tm) = self
                .waiter
                .wait_timeout(lock, timeout)
                .unwrap_or_else(|e| e.into_inner());
            if tm.timed_out() {
                return Err(Timeout {});
            }
            lock = l;
        }
        let mut retvl = Vec::with_capacity(lock.len());
        retvl.append(&mut lock);
        Ok(retvl)
    }
}

pub struct Timeout {}

pub struct AudioReciever {
    buffer: Arc<WaitableBuffer<i16>>,
    error_recv: crossbeam::Receiver<AssistantRsError>,
    stream: cpal::Stream,
}

impl Drop for AudioReciever {
    fn drop(&mut self) {
        if let Err(_e) = self.stream.pause() {
            //TODO: return the error?
        }
    }
}

impl AudioReciever {
    pub fn construct(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
    ) -> Result<Self, AssistantRsError> {
        let buffer = Arc::new(WaitableBuffer::<i16>::new());
        let handle = Arc::clone(&buffer);
        let (error_send, error_recv) = crossbeam::bounded::<AssistantRsError>(1);

        let stream = device
            .build_input_stream(
                &config,
                move |dt: &[i16], _cb| {
                    handle.push_slice(dt);
                },
                move |e| error_send.send(CpalError::from(e).into()).unwrap(),
            )
            .map_err(|e| CpalError::from(e))?;
        stream.play().map_err(|e| CpalError::from(e))?;
        let retvl = Self {
            buffer,
            error_recv,
            stream,
        };
        Ok(retvl)
    }
    pub fn wait_until(&self, target: usize) -> Result<Vec<i16>, AssistantRsError> {
        const POLL_LENGTH: Duration = Duration::from_millis(100);
        loop {
            match self.error_recv.try_recv() {
                Ok(err) => {
                    break Err(err);
                }
                Err(crossbeam::TryRecvError::Empty) => {}
                Err(crossbeam::TryRecvError::Disconnected) => todo!(),
            }
            if let Ok(ret) = self.buffer.wait_until_timeout(target, POLL_LENGTH) {
                break Ok(ret);
            }
        }
    }
}
