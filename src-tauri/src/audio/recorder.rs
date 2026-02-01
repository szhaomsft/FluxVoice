use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat};
use std::io::Cursor;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread;
use tokio::sync::oneshot;
use audiopus::{coder::Encoder, Application, Channels, SampleRate};
use ogg::writing::PacketWriteEndInfo;

const TARGET_SAMPLE_RATE: u32 = 16000; // Optimal for Azure Speech Service
const OPUS_FRAME_SIZE: usize = 960; // 60ms at 16kHz (recommended for voice)

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
        // Check if already recording - stop previous recording first
        {
            let is_recording = self.is_recording.lock().unwrap();
            if *is_recording {
                log::warn!("start_recording called but already recording - stopping previous recording first");
                drop(is_recording); // Release lock before stopping

                // Send stop signal to previous recording
                if let Some(sender) = self.stop_sender.take() {
                    let _ = sender.send(());
                }

                // Wait for previous recording to stop
                std::thread::sleep(std::time::Duration::from_millis(150));

                // Force reset the flag
                let mut is_rec = self.is_recording.lock().unwrap();
                *is_rec = false;
            }
        }

        // Clear previous buffer
        {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.clear();
        }

        // Clear any pending stop sender
        self.stop_sender = None;

        log::info!("Starting recording...");

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
                    if let Ok(mut recording) = is_recording.lock() {
                        *recording = false;
                    }
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
                    if let Ok(mut recording) = is_recording.lock() {
                        *recording = false;
                    }
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
                    if let Ok(mut recording) = is_recording.lock() {
                        *recording = false;
                    }
                    return;
                }
            };

            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to build stream: {}", e);
                    if let Ok(mut recording) = is_recording.lock() {
                        *recording = false;
                    }
                    return;
                }
            };

            if let Err(e) = stream.play() {
                log::error!("Failed to play stream: {}", e);
                if let Ok(mut recording) = is_recording.lock() {
                    *recording = false;
                }
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
        log::info!("stop_recording called");

        // Send stop signal
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }

        // Wait a bit for the thread to stop
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Set recording flag to false
        {
            let mut recording = self.is_recording.lock().unwrap();
            log::info!("Setting is_recording to false (was: {})", *recording);
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

        // Check minimum recording duration (at least 0.5 seconds = 8000 samples at 16kHz)
        const MIN_SAMPLES: usize = 8000;
        if resampled.len() < MIN_SAMPLES {
            log::warn!(
                "Recording too short: {} samples (minimum {}). Duration: {:.2}s",
                resampled.len(),
                MIN_SAMPLES,
                resampled.len() as f32 / TARGET_SAMPLE_RATE as f32
            );
            return Err(format!(
                "Recording too short ({:.1}s). Please hold the key longer.",
                resampled.len() as f32 / TARGET_SAMPLE_RATE as f32
            ));
        }

        // Convert to Opus/OGG format
        samples_to_opus(&resampled)
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

fn samples_to_opus(samples: &[f32]) -> Result<Vec<u8>, String> {
    // Create Opus encoder
    let mut encoder = Encoder::new(SampleRate::Hz16000, Channels::Mono, Application::Voip)
        .map_err(|e| format!("Failed to create Opus encoder: {:?}", e))?;

    // Set bitrate for voice (16kbps is good for speech)
    encoder.set_bitrate(audiopus::Bitrate::BitsPerSecond(16000))
        .map_err(|e| format!("Failed to set bitrate: {:?}", e))?;

    // Convert f32 samples to i16
    let i16_samples: Vec<i16> = samples
        .iter()
        .map(|s| (*s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16)
        .collect();

    // Create OGG container
    let mut cursor = Cursor::new(Vec::new());
    let serial = rand_serial();
    let mut packet_writer = ogg::writing::PacketWriter::new(&mut cursor);

    // Write Opus header (OpusHead)
    let opus_head = create_opus_head();
    packet_writer.write_packet(opus_head, serial, PacketWriteEndInfo::EndPage, 0)
        .map_err(|e| format!("Failed to write OpusHead: {}", e))?;

    // Write Opus comment header (OpusTags)
    let opus_tags = create_opus_tags();
    packet_writer.write_packet(opus_tags, serial, PacketWriteEndInfo::EndPage, 0)
        .map_err(|e| format!("Failed to write OpusTags: {}", e))?;

    // Encode audio in frames
    let mut granule_pos: u64 = 0;
    let mut encoded_buf = vec![0u8; 4000]; // Max Opus packet size

    for chunk in i16_samples.chunks(OPUS_FRAME_SIZE) {
        // Pad last frame if needed
        let frame: Vec<i16> = if chunk.len() < OPUS_FRAME_SIZE {
            let mut padded = chunk.to_vec();
            padded.resize(OPUS_FRAME_SIZE, 0);
            padded
        } else {
            chunk.to_vec()
        };

        let encoded_len = encoder.encode(&frame, &mut encoded_buf)
            .map_err(|e| format!("Failed to encode Opus frame: {:?}", e))?;

        // Granule position is in 48kHz samples (Opus standard), so multiply by 3 (48000/16000)
        granule_pos += (OPUS_FRAME_SIZE as u64) * 3;

        let is_last = chunk.len() < OPUS_FRAME_SIZE;
        let end_info = if is_last {
            PacketWriteEndInfo::EndStream
        } else {
            PacketWriteEndInfo::NormalPacket
        };

        packet_writer.write_packet(
            encoded_buf[..encoded_len].to_vec(),
            serial,
            end_info,
            granule_pos,
        ).map_err(|e| format!("Failed to write Opus packet: {}", e))?;
    }

    let result = cursor.into_inner();
    log::info!("Encoded {} samples to {} bytes Opus/OGG", samples.len(), result.len());

    Ok(result)
}

fn rand_serial() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u32)
        .unwrap_or(12345678)
}

fn create_opus_head() -> Vec<u8> {
    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");  // Magic signature
    head.push(1);                          // Version
    head.push(1);                          // Channel count (mono)
    head.extend_from_slice(&0u16.to_le_bytes());  // Pre-skip
    head.extend_from_slice(&48000u32.to_le_bytes()); // Original sample rate (Opus always uses 48kHz internally)
    head.extend_from_slice(&0i16.to_le_bytes());  // Output gain
    head.push(0);                          // Channel mapping family
    head
}

fn create_opus_tags() -> Vec<u8> {
    let mut tags = Vec::new();
    tags.extend_from_slice(b"OpusTags");  // Magic signature
    let vendor = b"FluxVoice";
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes()); // No user comments
    tags
}
