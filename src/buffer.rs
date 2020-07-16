use crate::error::{AssistantRsError, CpalError};
use cpal::traits::{DeviceTrait, StreamTrait};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// A buffer used to store speech data and monitor the beginning and end of speech.
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

    /// The time since the loader detected new speech.
    pub fn time_since_change(&self) -> Duration {
        let nanos =
            ((1_000_000_000u64) * (self.samples_since_change as u64)) / (self.sample_rate as u64);
        Duration::from_nanos(nanos)
    }

    /// Pushes new audio sample data to the model; returns whether or not the samples contained new speech information on success.
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
            // only push relevant data to our buffer
            if next_text != "" {
                self.raw_data.extend_from_slice(data);
            }
            self.samples_since_change += data.len();
            Ok(false)
        }
    }

    /// Gets the current transcript of the audio stored in this loader.
    pub fn current_text(&self) -> &str {
        &self.current_text
    }

    /// Gets the total number of samples that were `push`ed to this loader.
    pub fn num_samples(&self) -> usize {
        self.total_samples
    }

    /// "Finishes" the loader, returning the text transcription and raw audio data.
    pub fn finish(self) -> (String, Vec<i16>) {
        (self.current_text, self.raw_data)
    }
}

/// A buffer to pass data blocks of variable length between threads.
pub struct WaitableBuffer<T: Clone> {
    data: Mutex<Vec<T>>,
    waiter: Condvar,
}

impl<T: Clone> Default for WaitableBuffer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> WaitableBuffer<T> {
    /// Constructs a new `WaitableBuffer`.
    pub fn new() -> Self {
        Self {
            data: Mutex::new(Vec::new()),
            waiter: Condvar::new(),
        }
    }

    /// Pushes data into the buffer synchronously.
    pub fn push_slice(&self, data: &[T]) {
        let mut lock = self.data.lock().unwrap_or_else(|e| e.into_inner());
        lock.extend_from_slice(data);
        self.waiter.notify_all();
    }

    /// Blockes the thread until either the buffer reaches at least a length of `target` or the duration specified
    /// by `timeout` passes.
    ///
    /// On success, all data is taken out of the buffer and returned, even if there are more elements than `target`.
    /// On timeout, the method returns `Err(Timeout{})`.
    pub fn wait_until_timeout(&self, target: usize, timeout: Duration) -> Result<Vec<T>, Timeout> {
        let start = std::time::Instant::now();
        let end = start + timeout;
        let mut lock = self.data.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            // Loop since Condvars can be woken up randomly.
            if lock.len() >= target {
                break;
            }
            // Check if we timed out yet.
            let now = std::time::Instant::now();
            if now > end {
                return Err(Timeout {});
            }

            // Math how much time is left
            let time_left = end - now;
            let (l, tm) = self
                .waiter
                .wait_timeout(lock, time_left)
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

/// The error returned when `WaitableBuffer::wait_until_timeout` times out.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, thiserror::Error)]
#[error("Timed out.")]
pub struct Timeout {}

/// Manages recieving audio from the microphone.
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
    /// Builts a new `AudioReciever`, including building the internal buffers and starting the `cpal` input stream.
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
                move |dt: &[i16], _cb| handle.push_slice(dt),
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

    /// Waits until the current audio buffer reaches at least a certain length before returning that data.
    /// The entire buffer is returned, not just the number of samples specified by `target`.
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
