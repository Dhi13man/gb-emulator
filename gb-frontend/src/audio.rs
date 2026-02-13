use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::sync::{Arc, Mutex};

const BUFFER_SIZE: usize = 4096;

/// Ring buffer for passing audio samples from emulator to audio callback.
struct RingBuffer {
    buffer: Vec<(f32, f32)>,
    read_pos: usize,
    write_pos: usize,
    count: usize,
}

impl RingBuffer {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![(0.0, 0.0); size],
            read_pos: 0,
            write_pos: 0,
            count: 0,
        }
    }

    fn push(&mut self, sample: (f32, f32)) {
        if self.count < self.buffer.len() {
            self.buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.buffer.len();
            self.count += 1;
        }
        // Drop samples if buffer is full (avoid latency buildup)
    }

    fn pop(&mut self) -> (f32, f32) {
        if self.count > 0 {
            let sample = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.buffer.len();
            self.count -= 1;
            sample
        } else {
            (0.0, 0.0) // Silence on underrun
        }
    }
}

pub struct AudioPlayer {
    _stream: Stream,
    ring_buffer: Arc<Mutex<RingBuffer>>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or("No audio output device found")?;

        let config = device.default_output_config()?;
        let sample_format = config.sample_format();
        let config = config.into();

        let ring_buffer = Arc::new(Mutex::new(RingBuffer::new(BUFFER_SIZE)));
        let rb_clone = Arc::clone(&ring_buffer);

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _| {
                    let mut rb = rb_clone.lock().unwrap();
                    for frame in data.chunks_exact_mut(2) {
                        let (l, r) = rb.pop();
                        frame[0] = l;
                        frame[1] = r;
                    }
                },
                |err| eprintln!("Audio stream error: {err}"),
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data: &mut [i16], _| {
                    let mut rb = rb_clone.lock().unwrap();
                    for frame in data.chunks_exact_mut(2) {
                        let (l, r) = rb.pop();
                        frame[0] = (l * i16::MAX as f32) as i16;
                        frame[1] = (r * i16::MAX as f32) as i16;
                    }
                },
                |err| eprintln!("Audio stream error: {err}"),
                None,
            )?,
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |data: &mut [u16], _| {
                    let mut rb = rb_clone.lock().unwrap();
                    for frame in data.chunks_exact_mut(2) {
                        let (l, r) = rb.pop();
                        frame[0] = ((l + 1.0) * 0.5 * u16::MAX as f32) as u16;
                        frame[1] = ((r + 1.0) * 0.5 * u16::MAX as f32) as u16;
                    }
                },
                |err| eprintln!("Audio stream error: {err}"),
                None,
            )?,
            _ => return Err("Unsupported sample format".into()),
        };

        stream.play()?;

        Ok(Self {
            _stream: stream,
            ring_buffer,
        })
    }

    pub fn push_samples(&self, samples: &[(f32, f32)]) {
        let mut rb = self.ring_buffer.lock().unwrap();
        for &sample in samples {
            rb.push(sample);
        }
    }
}
