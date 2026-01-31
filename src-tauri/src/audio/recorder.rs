use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat};
use hound::{WavSpec, WavWriter};
use std::io::Cursor;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread;
use tokio::sync::oneshot;

const TARGET_SAMPLE_RATE: u32 = 16000; // Optimal for Azure Speech Service
const CHANNELS: u16 = 1; // Mono
const BITS_PER_SAMPLE: u16 = 16;

pub struct AudioRecorder {
    buffer: Arc<StdMutex<Vec<f32>>>,
    is_recording: Arc<StdMutex<bool>>,
    source_sample_rate: Arc<StdMutex<u32>>,
    source_channels: Arc<StdMutex<u16>>,
    stop_sender: Option<oneshot::Sender<()>>,
}

// Manually implement Send + Sync since we're not storing the Stream anymore
unsafe impl Send for AudioRecorder {}
unsafe impl Sync for AudioRecorder {}

impl AudioRecorder {
    pub fn new() -> Result<Self, String> {
        // Verify we have a default input device
        let host = cpal::default_host();
        let _device = host
            .default_input_device()
            .ok_or("No input device available")?;

        Ok(Self {
            buffer: Arc::new(StdMutex::new(Vec::new())),
            is_recording: Arc::new(StdMutex::new(false)),
            source_sample_rate: Arc::new(StdMutex::new(TARGET_SAMPLE_RATE)),
            source_channels: Arc::new(StdMutex::new(1)),
            stop_sender: None,
        })
    }

    pub fn start_recording(&mut self) -> Result<(), String> {
        // Clear previous buffer
        {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.clear();
        }

        // Check if already recording
        {
            let is_recording = self.is_recording.lock().unwrap();
            if *is_recording {
                return Err("Already recording".to_string());
            }
        }

        let buffer = Arc::clone(&self.buffer);
        let is_recording = Arc::clone(&self.is_recording);
        let source_sample_rate = Arc::clone(&self.source_sample_rate);
        let source_channels = Arc::clone(&self.source_channels);

        // Set recording flag
        {
            let mut recording = is_recording.lock().unwrap();
            *recording = true;
        }

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        self.stop_sender = Some(stop_tx);

        // Spawn a dedicated thread for audio recording
        // This avoids the Send requirement since the Stream stays in this thread
        thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    log::error!("No input device available");
                    return;
                }
            };

            log::info!(
                "Using audio input device: {}",
                device.name().unwrap_or_else(|_| "Unknown".to_string())
            );

            let supported_config = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed to get default input config: {}", e);
                    return;
                }
            };

            // Use the device's native config (don't override sample rate/channels)
            let config = supported_config.config();

            // Store the actual sample rate and channels for resampling later
            if let Ok(mut rate) = source_sample_rate.lock() {
                *rate = config.sample_rate.0;
            }
            if let Ok(mut ch) = source_channels.lock() {
                *ch = config.channels;
            }

            log::info!(
                "Audio config: {} Hz, {} channels, format: {:?}",
                config.sample_rate.0,
                config.channels,
                supported_config.sample_format()
            );

            let err_fn = |err| {
                log::error!("Stream error: {}", err);
            };

            let stream = match supported_config.sample_format() {
                SampleFormat::F32 => {
                    build_stream::<f32>(&device, &config, buffer.clone(), err_fn)
                }
                SampleFormat::I16 => {
                    build_stream::<i16>(&device, &config, buffer.clone(), err_fn)
                }
                SampleFormat::U16 => {
                    build_stream::<u16>(&device, &config, buffer.clone(), err_fn)
                }
                sample_format => {
                    log::error!("Unsupported sample format: {}", sample_format);
                    return;
                }
            };

            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to build stream: {}", e);
                    return;
                }
            };

            if let Err(e) = stream.play() {
                log::error!("Failed to play stream: {}", e);
                return;
            }

            log::info!("Recording started");

            // Block until we receive the stop signal
            let _ = stop_rx.blocking_recv();

            // Stream is dropped here, stopping the recording
            drop(stream);

            // Set recording flag to false
            if let Ok(mut recording) = is_recording.lock() {
                *recording = false;
            }

            log::info!("Recording thread stopped");
        });

        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<Vec<u8>, String> {
        // Send stop signal
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }

        // Wait a bit for the thread to stop
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Set recording flag to false
        {
            let mut recording = self.is_recording.lock().unwrap();
            *recording = false;
        }

        // Get buffer data and source info
        let buffer_data = {
            let buffer = self.buffer.lock().unwrap();
            buffer.clone()
        };

        let source_rate = *self.source_sample_rate.lock().unwrap();
        let source_channels = *self.source_channels.lock().unwrap();

        log::info!(
            "Recording stopped. Captured {} samples at {} Hz, {} channels",
            buffer_data.len(),
            source_rate,
            source_channels
        );

        // Convert to mono if needed
        let mono_data: Vec<f32> = if source_channels > 1 {
            buffer_data
                .chunks(source_channels as usize)
                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                .collect()
        } else {
            buffer_data
        };

        // Resample to target sample rate if needed
        let resampled = if source_rate != TARGET_SAMPLE_RATE {
            resample(&mono_data, source_rate, TARGET_SAMPLE_RATE)
        } else {
            mono_data
        };

        log::info!("Resampled to {} samples at {} Hz", resampled.len(), TARGET_SAMPLE_RATE);

        // Convert to WAV format
        samples_to_wav(&resampled)
    }

    pub fn get_audio_level(&self) -> f32 {
        let buffer = self.buffer.lock().unwrap();

        // Get last 1000 samples or all if less
        let samples_to_check = buffer.len().min(1000);
        if samples_to_check == 0 {
            return 0.0;
        }

        let recent_samples = &buffer[buffer.len() - samples_to_check..];

        // Calculate RMS (Root Mean Square)
        let sum_squares: f32 = recent_samples.iter().map(|s| s * s).sum();
        let rms = (sum_squares / samples_to_check as f32).sqrt();

        // Normalize to 0.0 - 1.0 range
        (rms * 10.0).min(1.0)
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<StdMutex<Vec<f32>>>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = buffer.lock() {
                    // Convert all samples to f32 (keep all channels, we'll convert to mono later)
                    for sample in data {
                        buf.push(f32::from_sample_(*sample));
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    Ok(stream)
}

/// Simple linear interpolation resampler
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(samples.len() - 1);
        let frac = src_idx - idx_floor as f64;

        let sample = samples[idx_floor] * (1.0 - frac as f32) + samples[idx_ceil] * frac as f32;
        output.push(sample);
    }

    output
}

fn samples_to_wav(samples: &[f32]) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::new());

    let spec = WavSpec {
        channels: CHANNELS,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: BITS_PER_SAMPLE,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer =
        WavWriter::new(&mut cursor, spec).map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    // Convert f32 samples to i16
    for sample in samples {
        let amplitude = (sample * i16::MAX as f32) as i16;
        writer
            .write_sample(amplitude)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    Ok(cursor.into_inner())
}
