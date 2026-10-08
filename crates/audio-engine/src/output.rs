use std::{cell::Cell, sync::{atomic::{AtomicU32, AtomicU64, Ordering}, Arc}, thread, time::Duration};

use anyhow::{bail, Context, Result};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, BufferSize, SampleFormat, Stream, StreamConfig};
use crossbeam_queue::ArrayQueue;

use crate::playback_state::{PlaybackClock, PlaybackStateSender};

#[cfg(test)]
mod tests {
    use super::SampleQueue;

    #[test]
    fn converts_f32_ring_to_i16_without_clipping() {
        let queue = SampleQueue::default();
        let mut out = vec![0_i16; 4];
        queue.push_interleaved([0.0, 1.0, -1.0, 0.5]);
        for sample in out.iter_mut() {
            let value = queue.fill_output_i16(sample);
            *sample = value;
        }
        assert_eq!(out[0], 0);
        assert_eq!(out[1], i16::MAX);
        assert_eq!(out[2], i16::MIN);
        assert!(out[3] > 0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LatencyProfile {
    #[default]
    Default,
    LowLatency,    // 512 frames (~11.6ms)
    Balanced,      // 1024 frames (~23.2ms)
    HighStability, // 2048 frames (~46.4ms)
    FixedFrames(u32),
}

impl LatencyProfile {
    pub fn all() -> &'static [LatencyProfile] {
        &[
            LatencyProfile::Default,
            LatencyProfile::LowLatency,
            LatencyProfile::Balanced,
            LatencyProfile::HighStability,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "System Default (Adaptive)",
            Self::LowLatency => "Low Latency (512 frames / ~11ms)",
            Self::Balanced => "Balanced (1024 frames / ~23ms)",
            Self::HighStability => "Safe Buffer (2048 frames / ~46ms)",
            Self::FixedFrames(_) => "Custom Frame Buffer",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::LowLatency => "Low (512)",
            Self::Balanced => "Balanced (1024)",
            Self::HighStability => "Safe (2048)",
            Self::FixedFrames(_) => "Custom",
        }
    }

    pub fn frames(&self) -> Option<u32> {
        match self {
            Self::Default => None,
            Self::LowLatency => Some(512),
            Self::Balanced => Some(1024),
            Self::HighStability => Some(2048),
            Self::FixedFrames(f) => Some(*f),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoundProfile {
    #[default]
    Flat,
    BassBoost,
    TrebleClarity,
    VocalPresence,
    ElectronicClub,
    WarmAnalog,
}

impl SoundProfile {
    pub fn all() -> &'static [SoundProfile] {
        &[
            SoundProfile::Flat,
            SoundProfile::BassBoost,
            SoundProfile::TrebleClarity,
            SoundProfile::VocalPresence,
            SoundProfile::ElectronicClub,
            SoundProfile::WarmAnalog,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Flat => "Flat (Pure Studio Reference)",
            Self::BassBoost => "Bass Boost (+6dB Sub-Bass & Punch)",
            Self::TrebleClarity => "Treble Clarity (+4dB Highs & Air)",
            Self::VocalPresence => "Vocal Presence (+3dB Mid-Range)",
            Self::ElectronicClub => "Club / Dance (V-Shaped Dynamic)",
            Self::WarmAnalog => "Warm Analog (Tube Saturation)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Flat => "Flat",
            Self::BassBoost => "Bass Boost",
            Self::TrebleClarity => "Treble",
            Self::VocalPresence => "Vocal",
            Self::ElectronicClub => "Club",
            Self::WarmAnalog => "Warm Analog",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Flat => "Bit-perfect uncolored frequency response matching studio master recordings.",
            Self::BassBoost => "Emphasizes frequencies below 200 Hz for powerful sub-bass and warm kick drums.",
            Self::TrebleClarity => "Enhances frequencies above 4 kHz for crisp cymbals, air, and high-frequency sparkle.",
            Self::VocalPresence => "Focuses on 800 Hz – 3.5 kHz to bring lead vocals, speech, and acoustic guitars forward.",
            Self::ElectronicClub => "Dynamic V-shaped EQ boosting sub-bass and sparkling highs for modern electronic & EDM.",
            Self::WarmAnalog => "Gentle high-frequency roll-off with subtle low-order harmonic warmth.",
        }
    }

    pub fn to_u32(self) -> u32 {
        match self {
            Self::Flat => 0,
            Self::BassBoost => 1,
            Self::TrebleClarity => 2,
            Self::VocalPresence => 3,
            Self::ElectronicClub => 4,
            Self::WarmAnalog => 5,
        }
    }

    pub fn from_u32(val: u32) -> Self {
        match val {
            1 => Self::BassBoost,
            2 => Self::TrebleClarity,
            3 => Self::VocalPresence,
            4 => Self::ElectronicClub,
            5 => Self::WarmAnalog,
            _ => Self::Flat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetLoudness {
    Streaming14,
    #[default]
    Balanced18,
    Broadcast23,
}

impl TargetLoudness {
    pub fn all() -> &'static [TargetLoudness] {
        &[
            TargetLoudness::Streaming14,
            TargetLoudness::Balanced18,
            TargetLoudness::Broadcast23,
        ]
    }

    pub fn target_lufs(&self) -> f64 {
        match self {
            Self::Streaming14 => -14.0,
            Self::Balanced18 => -18.0,
            Self::Broadcast23 => -23.0,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Streaming14 => "-14 LUFS (Streaming Standard - Spotify / YouTube)",
            Self::Balanced18 => "-18 LUFS (Kanono High-Dynamic Balanced Default)",
            Self::Broadcast23 => "-23 LUFS (EBU R128 European Broadcast Standard)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Streaming14 => "-14 LUFS (Streaming)",
            Self::Balanced18 => "-18 LUFS (Balanced)",
            Self::Broadcast23 => "-23 LUFS (Broadcast)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Streaming14 => "Matches target volume of modern music streaming services (Spotify, YouTube, Apple Music).",
            Self::Balanced18 => "Ideal balance preserving natural dynamic range with comfortable listening levels.",
            Self::Broadcast23 => "Strict European Broadcasting Union standard for wide dynamic classical and film audio.",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub device_name: String,
    pub host_api: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
    pub buffer_size_desc: String,
}

pub fn get_audio_device_info() -> AudioDeviceInfo {
    let host = cpal::default_host();
    let host_id_str = format!("{:?}", host.id());
    let host_api = if host_id_str.contains("Alsa") {
        "ALSA (PipeWire / PulseAudio compatibility)".to_string()
    } else {
        host_id_str
    };
    if let Some(device) = host.default_output_device() {
        let name = device.name().unwrap_or_else(|_| "Default Soundcard".to_string());
        if let Ok(cfg) = device.default_output_config() {
            let buf_desc = match cfg.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => format!("{min} - {max} frames"),
                cpal::SupportedBufferSize::Unknown => "Adaptive (Host Managed)".to_string(),
            };
            return AudioDeviceInfo {
                device_name: name,
                host_api,
                sample_rate: cfg.sample_rate().0,
                channels: cfg.channels(),
                sample_format: format!("{:?}", cfg.sample_format()),
                buffer_size_desc: buf_desc,
            };
        }
    }
    AudioDeviceInfo {
        device_name: "Default System Output Device".to_string(),
        host_api,
        sample_rate: 48000,
        channels: 2,
        sample_format: "F32".to_string(),
        buffer_size_desc: "Adaptive (Host Managed)".to_string(),
    }
}

/// Generates a smooth 440 Hz (A4) test chime with soft fade in/out.
pub fn generate_test_tone(sample_rate: u32, duration_secs: f32) -> Vec<f32> {
    let total_samples = (sample_rate as f32 * duration_secs) as usize;
    let mut out = Vec::with_capacity(total_samples * 2);
    let freq = 440.0_f32;
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let env = if t < 0.05 {
            t / 0.05
        } else if t > duration_secs - 0.2 {
            ((duration_secs - t) / 0.2).max(0.0)
        } else {
            1.0
        };
        let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.4 * env;
        out.push(s);
        out.push(s);
    }
    out
}

/// Generates a stereo channel identification test (Left chime, silence, Right chime).
pub fn generate_stereo_channel_test(sample_rate: u32) -> Vec<f32> {
    let mut out = Vec::new();
    // 1. Left channel only: 440 Hz for 0.7s
    let left_samples = (sample_rate as f32 * 0.7) as usize;
    for i in 0..left_samples {
        let t = i as f32 / sample_rate as f32;
        let env = if t < 0.05 { t / 0.05 } else if t > 0.55 { (0.7 - t) / 0.15 } else { 1.0 };
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.45 * env;
        out.push(s);
        out.push(0.0);
    }
    // 2. Silence for 0.25s
    let pause_samples = (sample_rate as f32 * 0.25) as usize;
    for _ in 0..pause_samples {
        out.push(0.0);
        out.push(0.0);
    }
    // 3. Right channel only: 660 Hz for 0.7s
    let right_samples = (sample_rate as f32 * 0.7) as usize;
    for i in 0..right_samples {
        let t = i as f32 / sample_rate as f32;
        let env = if t < 0.05 { t / 0.05 } else if t > 0.55 { (0.7 - t) / 0.15 } else { 1.0 };
        let s = (2.0 * std::f32::consts::PI * 660.0 * t).sin() * 0.45 * env;
        out.push(0.0);
        out.push(s);
    }
    out
}

thread_local! {
    static EQ_STATE_L: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
    static EQ_STATE_R: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
}

/// Applies real-time sound coloration / tone sculpting in-place with zero latency and zero heap allocation.
pub fn apply_sound_profile(samples: &mut [f32], channels: usize, profile: SoundProfile) {
    if profile == SoundProfile::Flat || samples.is_empty() {
        return;
    }
    let mut state_l = EQ_STATE_L.get();
    let mut state_r = EQ_STATE_R.get();

    let step = channels.max(1);
    let mut i = 0;
    while i < samples.len() {
        // Left (or mono)
        let in_l = samples[i];
        state_l.0 += 0.05 * (in_l - state_l.0);
        state_l.1 += 0.30 * (in_l - state_l.1);

        let lp_bass = state_l.0;
        let band_mid = state_l.1 - state_l.0;
        let hp_treble = in_l - state_l.1;

        let out_l = match profile {
            SoundProfile::Flat => in_l,
            SoundProfile::BassBoost => in_l + 0.60 * lp_bass,
            SoundProfile::TrebleClarity => in_l + 0.55 * hp_treble,
            SoundProfile::VocalPresence => in_l + 0.50 * band_mid,
            SoundProfile::ElectronicClub => in_l + 0.50 * lp_bass + 0.40 * hp_treble - 0.15 * band_mid,
            SoundProfile::WarmAnalog => {
                let warm = 0.85 * in_l + 0.15 * state_l.1;
                let drive = warm * 1.15;
                drive / (1.0 + drive.abs() * 0.3)
            }
        };
        samples[i] = out_l.clamp(-1.0, 1.0);

        // Right (if stereo)
        if channels >= 2 && i + 1 < samples.len() {
            let in_r = samples[i + 1];
            state_r.0 += 0.05 * (in_r - state_r.0);
            state_r.1 += 0.30 * (in_r - state_r.1);

            let lp_bass_r = state_r.0;
            let band_mid_r = state_r.1 - state_r.0;
            let hp_treble_r = in_r - state_r.1;

            let out_r = match profile {
                SoundProfile::Flat => in_r,
                SoundProfile::BassBoost => in_r + 0.60 * lp_bass_r,
                SoundProfile::TrebleClarity => in_r + 0.55 * hp_treble_r,
                SoundProfile::VocalPresence => in_r + 0.50 * band_mid_r,
                SoundProfile::ElectronicClub => in_r + 0.50 * lp_bass_r + 0.40 * hp_treble_r - 0.15 * band_mid_r,
                SoundProfile::WarmAnalog => {
                    let warm = 0.85 * in_r + 0.15 * state_r.1;
                    let drive = warm * 1.15;
                    drive / (1.0 + drive.abs() * 0.3)
                }
            };
            samples[i + 1] = out_r.clamp(-1.0, 1.0);
        }

        i += step;
    }

    EQ_STATE_L.set(state_l);
    EQ_STATE_R.set(state_r);
}

/// Interleaved 32-bit floating point samples shared by the decode worker and callback.
#[derive(Clone)]
pub struct SampleQueue(Arc<ArrayQueue<f32>>);

impl Default for SampleQueue {
    fn default() -> Self {
        Self::with_capacity(48_000 * 2 * 10)
    }
}

impl SampleQueue {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Arc::new(ArrayQueue::new(capacity)))
    }

    /// Blocks only the producer when the fixed PCM ring is full; never call from CPAL.
    pub fn push_interleaved(&self, samples: impl IntoIterator<Item = f32>) {
        self.push_interleaved_cancellable(samples, || true);
    }

    /// Returns false when playback was superseded while a decode worker was filling the ring.
    pub fn push_interleaved_cancellable(
        &self,
        samples: impl IntoIterator<Item = f32>,
        mut should_continue: impl FnMut() -> bool,
    ) -> bool {
        let mut count = 0usize;
        for sample in samples {
            count += 1;
            if count.is_multiple_of(512) && !should_continue() {
                return false;
            }
            while self.0.push(sample).is_err() {
                if !should_continue() {
                    return false;
                }
                thread::sleep(Duration::from_millis(2));
            }
        }
        true
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&self) {
        while self.0.pop().is_some() {}
    }

    /// The allocation-free PCM transfer used by the CPAL output callback.
    /// Returns the number of samples actually drained from the queue.
    #[inline]
    pub fn fill_output(&self, output: &mut [f32]) -> usize {
        let mut popped = 0usize;
        for sample in output.iter_mut() {
            if let Some(val) = self.0.pop() {
                *sample = val;
                popped += 1;
            } else {
                *sample = 0.0;
            }
        }
        popped
    }

    #[inline]
    pub fn fill_output_i16(&self, sample: &mut i16) -> i16 {
        let value = self.0.pop().unwrap_or(0.0).clamp(-1.0, 1.0);
        let pcm = if value < 0.0 {
            (value * 32768.0).round().clamp(-32768.0, 32767.0) as i16
        } else {
            (value * 32767.0).round().clamp(-32768.0, 32767.0) as i16
        };
        *sample = pcm;
        pcm
    }
}

/// Keeps the CPAL stream alive. The callback does no decoding or allocation.
pub struct AudioOutput {
    _stream: Stream,
    pub queue: SampleQueue,
    pub config: StreamConfig,
    pub clock: PlaybackClock,
    pub volume: Arc<AtomicU32>,
    pub sound_profile: Arc<AtomicU32>,
    pub last_position_update: Arc<AtomicU64>,
}

impl AudioOutput {
    pub fn open_default(state_sender: PlaybackStateSender) -> Result<Self> {
        Self::open_default_tuned(state_sender, LatencyProfile::Default)
    }

    /// Requests a hardware buffer size for PipeWire's ALSA compatibility layer or JACK.
    /// The audio server remains authoritative and may reject unavailable fixed sizes.
    pub fn open_default_tuned(state_sender: PlaybackStateSender, latency: LatencyProfile) -> Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .context("no default audio output device")?;
        let (mut config, sample_format) = if let Ok(default_cfg) = device.default_output_config() {
            let format = default_cfg.sample_format();
            (default_cfg.into(), format)
        } else {
            let configs: Vec<_> = device.supported_output_configs().context("failed to inspect output configurations")?.collect();
            let preferred = configs
                .iter()
                .find(|config| config.channels() == 2 && config.sample_format() == SampleFormat::F32)
                .or_else(|| configs.iter().find(|config| config.channels() == 2 && matches!(config.sample_format(), SampleFormat::I16 | SampleFormat::U16)))
                .or_else(|| configs.iter().find(|config| config.sample_format() == SampleFormat::F32))
                .or_else(|| configs.first())
                .context("default output device has no usable stream configuration")?;
            let sample_format = preferred.sample_format();
            let sample_rate = if 44100 >= preferred.min_sample_rate().0 && 44100 <= preferred.max_sample_rate().0 {
                cpal::SampleRate(44100)
            } else if 48000 >= preferred.min_sample_rate().0 && 48000 <= preferred.max_sample_rate().0 {
                cpal::SampleRate(48000)
            } else {
                preferred.min_sample_rate()
            };
            (preferred.with_sample_rate(sample_rate).config(), sample_format)
        };

        if let Some(frames) = latency.frames() {
            config.buffer_size = BufferSize::Fixed(frames);
        }
        let queue = SampleQueue::with_capacity(usize::from(config.channels) * config.sample_rate.0 as usize * 10);
        let callback_queue = queue.clone();
        let clock = PlaybackClock::default();
        let callback_clock = clock.clone();
        let volume = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let callback_volume = Arc::clone(&volume);
        let sound_profile = Arc::new(AtomicU32::new(0));
        let callback_sound_profile = Arc::clone(&sound_profile);
        let last_position_update = Arc::new(AtomicU64::new(0));
        let callback_last_position_update = Arc::clone(&last_position_update);
        let channels = usize::from(config.channels);
        let sample_rate = config.sample_rate.0;
        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |output: &mut [f32], _| {
                    let popped = callback_queue.fill_output(output);
                    let profile_id = callback_sound_profile.load(Ordering::Relaxed);
                    let profile = SoundProfile::from_u32(profile_id);
                    if profile != SoundProfile::Flat && popped > 0 {
                        apply_sound_profile(&mut output[..popped], channels, profile);
                    }
                    let vol = f32::from_bits(callback_volume.load(Ordering::Relaxed));
                    if (vol - 1.0).abs() > 0.001 {
                        for sample in output.iter_mut() {
                            *sample *= vol;
                        }
                    }
                    let actual_frames = (popped / channels) as u64;
                    if actual_frames > 0 {
                        let played_frames = callback_clock.advance(actual_frames);
                        let update_interval = u64::from(sample_rate / 30).max(1);
                        let last_update = callback_last_position_update.load(Ordering::Relaxed);
                        if last_update == 0 || played_frames < last_update || played_frames.saturating_sub(last_update) >= update_interval {
                            callback_last_position_update.store(played_frames, Ordering::Relaxed);
                            state_sender.publish_position(callback_clock.position(sample_rate));
                            let win = output.len().min(512);
                            if win > 0 {
                                state_sender.publish_visualizer_pcm(Arc::from(&output[..win]));
                            }
                        }
                    }
                },
                move |error| eprintln!("audio output error: {error}"),
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |output: &mut [i16], _| {
                    let vol = f32::from_bits(callback_volume.load(Ordering::Relaxed));
                    let mut popped = 0usize;
                    for sample in output.iter_mut() {
                        if let Some(val) = callback_queue.0.pop() {
                            popped += 1;
                            let value = (val.clamp(-1.0, 1.0) * vol).clamp(-1.0, 1.0);
                            let pcm = if value < 0.0 {
                                (value * 32768.0).round().clamp(-32768.0, 32767.0) as i16
                            } else {
                                (value * 32767.0).round().clamp(-32768.0, 32767.0) as i16
                            };
                            *sample = pcm;
                        } else {
                            *sample = 0;
                        }
                    }
                    let actual_frames = (popped / channels) as u64;
                    if actual_frames > 0 {
                        let played_frames = callback_clock.advance(actual_frames);
                        let update_interval = u64::from(sample_rate / 30).max(1);
                        let last_update = callback_last_position_update.load(Ordering::Relaxed);
                        if last_update == 0 || played_frames < last_update || played_frames.saturating_sub(last_update) >= update_interval {
                            callback_last_position_update.store(played_frames, Ordering::Relaxed);
                            state_sender.publish_position(callback_clock.position(sample_rate));
                            let win = output.len().min(512);
                            if win > 0 {
                                let pcm_f32: Vec<f32> = output[..win].iter().map(|&s| s as f32 / 32768.0).collect();
                                state_sender.publish_visualizer_pcm(Arc::from(pcm_f32.as_slice()));
                            }
                        }
                    }
                },
                move |error| eprintln!("audio output error: {error}"),
                None,
            )?,
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |output: &mut [u16], _| {
                    let vol = f32::from_bits(callback_volume.load(Ordering::Relaxed));
                    let mut popped = 0usize;
                    for sample in output.iter_mut() {
                        if let Some(val) = callback_queue.0.pop() {
                            popped += 1;
                            let value = val.clamp(-1.0, 1.0) * vol;
                            *sample = ((value + 1.0) * u16::MAX as f32 / 2.0).round() as u16;
                        } else {
                            *sample = 32768;
                        }
                    }
                    let actual_frames = (popped / channels) as u64;
                    if actual_frames > 0 {
                        let played_frames = callback_clock.advance(actual_frames);
                        let update_interval = u64::from(sample_rate / 30).max(1);
                        let last_update = callback_last_position_update.load(Ordering::Relaxed);
                        if last_update == 0 || played_frames < last_update || played_frames.saturating_sub(last_update) >= update_interval {
                            callback_last_position_update.store(played_frames, Ordering::Relaxed);
                            state_sender.publish_position(callback_clock.position(sample_rate));
                            let win = output.len().min(512);
                            if win > 0 {
                                let pcm_f32: Vec<f32> = output[..win].iter().map(|&s| (s as f32 / 32767.5) - 1.0).collect();
                                state_sender.publish_visualizer_pcm(Arc::from(pcm_f32.as_slice()));
                            }
                        }
                    }
                },
                move |error| eprintln!("audio output error: {error}"),
                None,
            )?,
            sample_format => bail!("unsupported audio format: {sample_format:?}"),
        };
        let _ = stream.pause();
        Ok(Self {
            _stream: stream,
            queue,
            config,
            clock,
            volume,
            sound_profile,
            last_position_update,
        })
    }

    /// Append a decoded track without clearing queued samples to preserve gapless order.
    pub fn enqueue_track(&self, interleaved_f32: Vec<f32>) -> Result<()> {
        if interleaved_f32.is_empty() {
            bail!("decoded track contained no samples");
        }
        self.queue.push_interleaved(interleaved_f32);
        Ok(())
    }

    pub fn elapsed(&self) -> Duration {
        self.clock.position(self.config.sample_rate.0)
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn played_frames(&self) -> u64 {
        self.clock.played_frames()
    }

    pub fn play(&self) -> Result<()> {
        self._stream.play()?;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self._stream.pause()?;
        Ok(())
    }

    pub fn reset_position(&self) {
        self.clock.reset();
        self.last_position_update.store(0, Ordering::Relaxed);
    }

    pub fn set_position(&self, position: Duration) {
        let frames = (position.as_secs_f64() * f64::from(self.config.sample_rate.0)) as u64;
        self.clock.set_frames(frames);
        self.last_position_update.store(frames, Ordering::Relaxed);
    }

    pub fn set_volume(&self, vol: f32) {
        self.volume.store(vol.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    pub fn set_sound_profile(&self, profile: SoundProfile) {
        self.sound_profile.store(profile.to_u32(), Ordering::Relaxed);
    }

    pub fn sound_profile(&self) -> SoundProfile {
        SoundProfile::from_u32(self.sound_profile.load(Ordering::Relaxed))
    }

    /// Clears the output queue and plays a diagnostics audio test buffer immediately.
    pub fn play_test_buffer(&self, samples: Vec<f32>) -> Result<()> {
        self.queue.clear();
        self.reset_position();
        self.enqueue_track(samples)?;
        self.play()
    }
}