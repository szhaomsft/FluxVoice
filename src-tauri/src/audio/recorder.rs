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

    /// Create a dummy recorder when no audio input device is available.
    /// Recording will fail at runtime but the app can still start.
    pub fn new_dummy() -> Self {
        Self {
            buffer: Arc::new(StdMutex::new(Vec::new())),
            is_recording: Arc::new(StdMutex::new(false)),
            source_sample_rate: Arc::new(StdMutex::new(TARGET_SAMPLE_RATE)),
            source_channels: Arc::new(StdMutex::new(1)),
            stop_sender: None,
        }
    }

    pub fn start_recording(&mut self) -> Result<(), String> {
        // Check if already recording - stop previous recording first
        {
            let is_recording = self.is_recording.lock().unwrap();
            if *is_recording {
                log::warn!("start_recording called but already recording - stopping previous recording first");
                println!(">>> WARN: start_recording called but already recording");
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
            let prev_len = buffer.len();
            buffer.clear();
            log::info!("Cleared previous buffer ({} samples)", prev_len);
            println!(">>> Cleared previous buffer ({} samples)", prev_len);
        }

        // Clear any pending stop sender
        self.stop_sender = None;

        log::info!("Starting recording...");
        println!(">>> Starting recording...");

        let buffer = Arc::clone(&self.buffer);
        let is_recording = Arc::clone(&self.is_recording);
        let source_sample_rate = Arc::clone(&self.source_sample_rate);
        let source_channels = Arc::clone(&self.source_channels);

        // Set recording flag
        {
            let mut recording = is_recording.lock().unwrap();
            *recording = true;
            log::info!("Recording flag set to true");
            println!(">>> Recording flag set to true");
        }

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        self.stop_sender = Some(stop_tx);

        // Spawn a dedicated thread for audio recording
        // This avoids the Send requirement since the Stream stays in this thread
        thread::spawn(move || {
            println!(">>> Recording thread started");
            log::info!("Recording thread started");

            let host = cpal::default_host();
            println!(">>> Audio host: {:?}", host.id());

            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    log::error!("No input device available");
                    println!(">>> ERROR: No input device available");
                    if let Ok(mut recording) = is_recording.lock() {
                        *recording = false;
                    }
                    return;
                }
            };

            let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
            log::info!("Using audio input device: {}", device_name);
            println!(">>> Using audio input device: {}", device_name);

            let supported_config = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed to get default input config: {}", e);
                    println!(">>> ERROR: Failed to get default input config: {}", e);
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
            println!(
                ">>> Audio config: {} Hz, {} channels, format: {:?}",
                config.sample_rate.0,
                config.channels,
                supported_config.sample_format()
            );

            let err_fn = |err| {
                log::error!("Stream error: {}", err);
                println!(">>> Stream error: {}", err);
            };

            let buffer_for_callback = buffer.clone();
            let sample_counter = Arc::new(StdMutex::new(0usize));
            let sample_counter_for_callback = sample_counter.clone();

            let stream = match supported_config.sample_format() {
                SampleFormat::F32 => {
                    build_stream_with_logging::<f32>(&device, &config, buffer_for_callback, sample_counter_for_callback, err_fn)
                }
                SampleFormat::I16 => {
                    build_stream_with_logging::<i16>(&device, &config, buffer_for_callback, sample_counter_for_callback, err_fn)
                }
                SampleFormat::U16 => {
                    build_stream_with_logging::<u16>(&device, &config, buffer_for_callback, sample_counter_for_callback, err_fn)
                }
                sample_format => {
                    log::error!("Unsupported sample format: {}", sample_format);
                    println!(">>> ERROR: Unsupported sample format: {}", sample_format);
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
                    println!(">>> ERROR: Failed to build stream: {}", e);
                    if let Ok(mut recording) = is_recording.lock() {
                        *recording = false;
                    }
                    return;
                }
            };

            if let Err(e) = stream.play() {
                log::error!("Failed to play stream: {}", e);
                println!(">>> ERROR: Failed to play stream: {}", e);
                if let Ok(mut recording) = is_recording.lock() {
                    *recording = false;
                }
                return;
            }

            log::info!("Recording started - stream is playing");
            println!(">>> Recording started - stream is playing");

            // Block until we receive the stop signal
            let _ = stop_rx.blocking_recv();

            // Log final sample count
            if let Ok(count) = sample_counter.lock() {
                println!(">>> Recording stopping - total samples collected in callbacks: {}", *count);
                log::info!("Recording stopping - total samples collected in callbacks: {}", *count);
            }

            // Stream is dropped here, stopping the recording
            drop(stream);

            // Set recording flag to false
            if let Ok(mut recording) = is_recording.lock() {
                *recording = false;
            }

            log::info!("Recording thread stopped");
            println!(">>> Recording thread stopped");
        });

        Ok(())
    }

    pub fn stop_recording(&mut self, use_mp3: bool) -> Result<Vec<u8>, String> {
        let started = std::time::Instant::now();
        log::info!("stop_recording called");
        println!(">>> stop_recording called");

        // Send stop signal
        if let Some(sender) = self.stop_sender.take() {
            println!(">>> Sending stop signal to recording thread");
            let _ = sender.send(());
        } else {
            println!(">>> WARNING: No stop sender available - recording may not have started properly");
            log::warn!("No stop sender available - recording may not have started properly");
        }

        // Wait a bit for the thread to stop
        std::thread::sleep(std::time::Duration::from_millis(100));
        println!("[latency] recorder_stop_wait_ms={:.1}", started.elapsed().as_secs_f64() * 1000.0);
        let preparation_started = std::time::Instant::now();

        // Set recording flag to false
        {
            let mut recording = self.is_recording.lock().unwrap();
            log::info!("Setting is_recording to false (was: {})", *recording);
            println!(">>> Setting is_recording to false (was: {})", *recording);
            *recording = false;
        }

        // Get buffer data and source info
        let buffer_data = {
            let buffer = self.buffer.lock().unwrap();
            println!(">>> Buffer contains {} samples", buffer.len());
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
        println!(
            ">>> Recording stopped. Captured {} samples at {} Hz, {} channels",
            buffer_data.len(),
            source_rate,
            source_channels
        );

        // Check if we got any data at all
        if buffer_data.is_empty() {
            println!(">>> ERROR: No audio data captured!");
            log::error!("No audio data captured!");
            return Err("No audio data captured. Microphone may not be working.".to_string());
        }

        // Convert to mono if needed
        let mono_data: Vec<f32> = if source_channels > 1 {
            println!(">>> Converting {} channels to mono", source_channels);
            buffer_data
                .chunks(source_channels as usize)
                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                .collect()
        } else {
            buffer_data
        };

        println!(">>> Mono data: {} samples", mono_data.len());

        // Resample to target sample rate if needed
        let resampled = if source_rate != TARGET_SAMPLE_RATE {
            println!(">>> Resampling from {} Hz to {} Hz", source_rate, TARGET_SAMPLE_RATE);
            resample(&mono_data, source_rate, TARGET_SAMPLE_RATE)
        } else {
            mono_data
        };

        log::info!("Resampled to {} samples at {} Hz", resampled.len(), TARGET_SAMPLE_RATE);
        println!(">>> Resampled to {} samples at {} Hz (duration: {:.2}s)",
            resampled.len(),
            TARGET_SAMPLE_RATE,
            resampled.len() as f32 / TARGET_SAMPLE_RATE as f32
        );

        // Check minimum recording duration (at least 0.5 seconds = 8000 samples at 16kHz)
        const MIN_SAMPLES: usize = 8000;
        if resampled.len() < MIN_SAMPLES {
            log::warn!(
                "Recording too short: {} samples (minimum {}). Duration: {:.2}s",
                resampled.len(),
                MIN_SAMPLES,
                resampled.len() as f32 / TARGET_SAMPLE_RATE as f32
            );
            println!(
                ">>> WARNING: Recording too short: {} samples (minimum {}). Duration: {:.2}s",
                resampled.len(),
                MIN_SAMPLES,
                resampled.len() as f32 / TARGET_SAMPLE_RATE as f32
            );
            return Err(format!(
                "Recording too short ({:.1}s). Please hold the key longer.",
                resampled.len() as f32 / TARGET_SAMPLE_RATE as f32
            ));
        }

        // MAI-Transcribe-2 does not accept Opus/OGG.
        println!("[latency] audio_copy_mono_resample_ms={:.1}", preparation_started.elapsed().as_secs_f64() * 1000.0);
        let encoding_started = std::time::Instant::now();
        let result = if use_mp3 { samples_to_mp3(&resampled) } else { samples_to_opus(&resampled) };
        println!("[latency] audio_encoding_ms={:.1} format={} success={}", encoding_started.elapsed().as_secs_f64() * 1000.0, if use_mp3 { "mp3" } else { "opus" }, result.is_ok());
        if let Ok(ref data) = result {
            println!(">>> Encoded successfully: {} bytes", data.len());
        }
        result
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

fn build_stream_with_logging<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<StdMutex<Vec<f32>>>,
    sample_counter: Arc<StdMutex<usize>>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use std::time::Instant;
    let start_time = Instant::now();
    let last_log = Arc::new(StdMutex::new(Instant::now()));
    let callback_count = Arc::new(StdMutex::new(0usize));

    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = buffer.lock() {
                    let prev_len = buf.len();
                    // Convert all samples to f32 (keep all channels, we'll convert to mono later)
                    for sample in data {
                        buf.push(f32::from_sample_(*sample));
                    }

                    // Update sample counter
                    if let Ok(mut count) = sample_counter.lock() {
                        *count += data.len();
                    }

                    // Update callback count
                    if let Ok(mut count) = callback_count.lock() {
                        *count += 1;
                    }

                    // Log periodically (every 1 second)
                    if let Ok(mut last) = last_log.lock() {
                        if last.elapsed().as_secs() >= 1 {
                            let elapsed = start_time.elapsed().as_secs_f32();
                            let callbacks = callback_count.lock().map(|c| *c).unwrap_or(0);
                            println!(
                                ">>> Audio callback: +{} samples, total {} samples, {:.1}s elapsed, {} callbacks",
                                data.len(),
                                buf.len(),
                                elapsed,
                                callbacks
                            );
                            *last = Instant::now();
                        }
                    }

                    // Log first callback
                    if prev_len == 0 && !data.is_empty() {
                        println!(">>> First audio callback received: {} samples", data.len());
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

fn samples_to_mp3(samples: &[f32]) -> Result<Vec<u8>, String> {
    use mp3lame_encoder::{Bitrate, Builder, FlushGap, Mode, MonoPcm, Quality};

    let mut encoder = Builder::new()
        .ok_or_else(|| "Failed to allocate MP3 encoder".to_string())?
        .with_num_channels(1)
        .and_then(|builder| builder.with_mode(Mode::Mono))
        .and_then(|builder| builder.with_sample_rate(TARGET_SAMPLE_RATE))
        .and_then(|builder| builder.with_output_sample_rate(std::num::NonZeroU32::new(TARGET_SAMPLE_RATE)))
        .and_then(|builder| builder.with_brate(Bitrate::Kbps48))
        .and_then(|builder| builder.with_quality(Quality::Good))
        .and_then(|builder| builder.with_to_write_vbr_tag(false))
        .and_then(Builder::build)
        .map_err(|error| format!("Failed to configure MP3 encoder: {}", error))?;

    let pcm: Vec<i16> = samples.iter()
        .map(|sample| (*sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16)
        .collect();
    let mut output = Vec::with_capacity(mp3lame_encoder::max_required_buffer_size(pcm.len()));
    encoder.encode_to_vec(MonoPcm(&pcm), &mut output)
        .map_err(|error| format!("Failed to encode MP3 recording: {}", error))?;
    output.reserve(7200);
    // FlushGap emits buffered samples and pads the final frame instead of discarding the tail.
    encoder.flush_to_vec::<FlushGap>(&mut output)
        .map_err(|error| format!("Failed to finalize MP3 recording: {}", error))?;
    Ok(output)
}

#[cfg(test)]
fn samples_to_wav(samples: &[f32]) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::new(&mut cursor, spec)
        .map_err(|error| format!("Failed to create WAV recording: {}", error))?;
    for sample in samples {
        let pcm = (*sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer.write_sample(pcm)
            .map_err(|error| format!("Failed to write WAV recording: {}", error))?;
    }
    writer.finalize().map_err(|error| format!("Failed to finalize WAV recording: {}", error))?;
    Ok(cursor.into_inner())
}

fn samples_to_opus(samples: &[f32]) -> Result<Vec<u8>, String> {
    println!(">>> samples_to_opus: input {} f32 samples", samples.len());

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

    println!(">>> samples_to_opus: converted to {} i16 samples", i16_samples.len());

    // Check if samples are all zeros (silence)
    let non_zero_count = i16_samples.iter().filter(|&&s| s != 0).count();
    let max_sample = i16_samples.iter().map(|s| s.abs()).max().unwrap_or(0);
    println!(">>> samples_to_opus: non-zero samples: {}, max amplitude: {}", non_zero_count, max_sample);

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
    let mut frame_count = 0;
    let mut total_encoded_bytes = 0;
    let total_frames = (i16_samples.len() + OPUS_FRAME_SIZE - 1) / OPUS_FRAME_SIZE;

    println!(">>> samples_to_opus: will encode {} frames (OPUS_FRAME_SIZE={})", total_frames, OPUS_FRAME_SIZE);

    for (idx, chunk) in i16_samples.chunks(OPUS_FRAME_SIZE).enumerate() {
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

        frame_count += 1;
        total_encoded_bytes += encoded_len;

        // Granule position is in 48kHz samples (Opus standard), so multiply by 3 (48000/16000)
        granule_pos += (OPUS_FRAME_SIZE as u64) * 3;

        let is_last = idx == total_frames - 1;
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

    println!(">>> samples_to_opus: encoded {} frames, {} bytes of audio data", frame_count, total_encoded_bytes);

    let result = cursor.into_inner();
    log::info!("Encoded {} samples to {} bytes Opus/OGG", samples.len(), result.len());
    println!(">>> samples_to_opus: final OGG file size: {} bytes", result.len());

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp3_encoding_is_decodable_mono_16khz_48kbps_and_smaller_than_wav() {
        let samples: Vec<f32> = (0..160000)
            .map(|index| (index as f32 * 440.0 * std::f32::consts::TAU / 16000.0).sin() * 0.4)
            .collect();
        let started = std::time::Instant::now();
        let audio = samples_to_mp3(&samples).unwrap();
        let encoding_ms = started.elapsed().as_secs_f64() * 1000.0;
        let wav = samples_to_wav(&samples).unwrap();
        assert!(audio.len() < wav.len() / 4, "MP3 should be less than 25% of WAV size");
        println!("[mp3-check] duration_secs=10 mp3_bytes={} wav_bytes={} encoding_ms={:.1}", audio.len(), wav.len(), encoding_ms);

        let mut decoder = minimp3::Decoder::new(Cursor::new(audio));
        let mut decoded = Vec::new();
        loop {
            match decoder.next_frame() {
                Ok(frame) => {
                    assert_eq!(frame.sample_rate, 16000);
                    assert_eq!(frame.channels, 1);
                    assert_eq!(frame.bitrate, 48);
                    decoded.extend(frame.data);
                }
                Err(minimp3::Error::Eof) => break,
                Err(error) => panic!("Invalid encoded MP3: {:?}", error),
            }
        }
        assert!(decoded.len() >= samples.len(), "Final audio samples were not flushed");
        assert!(decoded.len() <= samples.len() + 2304, "Unexpected encoder delay/padding");
        assert!(decoded.iter().any(|sample| sample.abs() > 1000), "Encoded audio became silent");
    }

    #[test]
    fn mp3_flush_preserves_short_and_partial_frames() {
        for length in [8000, 12345] {
            let audio = samples_to_mp3(&vec![0.25; length]).unwrap();
            let mut decoder = minimp3::Decoder::new(Cursor::new(audio));
            let mut sample_count = 0;
            loop {
                match decoder.next_frame() {
                    Ok(frame) => sample_count += frame.data.len(),
                    Err(minimp3::Error::Eof) => break,
                    Err(error) => panic!("Invalid short MP3: {:?}", error),
                }
            }
            assert!(sample_count >= length, "Short recording was truncated");
            assert!(sample_count <= length + 2304);
        }
    }

    #[test]
    fn wav_encoding_preserves_duration_and_pcm_format() {
        let audio = samples_to_wav(&vec![0.25; 16000]).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(audio)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(reader.spec().sample_format, hound::SampleFormat::Int);
        assert_eq!(reader.duration(), 16000);
        assert!(reader.samples::<i16>().all(|sample| sample.unwrap() == 8191));
    }

    #[test]
    fn wav_encoding_clamps_out_of_range_samples() {
        let audio = samples_to_wav(&[-2.0, 0.0, 2.0]).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(audio)).unwrap();
        let samples: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
        assert_eq!(samples, vec![i16::MIN, 0, i16::MAX]);
    }

    #[test]
    fn existing_opus_encoding_remains_available() {
        let audio = samples_to_opus(&vec![0.0; 8000]).unwrap();
        assert!(audio.starts_with(b"OggS"));
        assert!(audio.windows(8).any(|window| window == b"OpusHead"));
    }
}
