pub mod midi_channel;
pub mod sample;
pub mod voice;

use midi_channel::MidiChannel;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use sample::{Sample, SampleData};
use voice::Voice;

pub const MAX_POLYPHONY: u32 = 4 * 1024 * 1024 * (1024 / std::mem::size_of::<Voice>() as u32);

pub const KSYNTH_BUILD_GIT_COMMIT_HASH: &str = env!("KSYNTH_BUILD_GIT_COMMIT_HASH");

const FADE_OUT_DURATION: Duration = Duration::from_millis(100);

const fn precompute_velocity_lut() -> [f32; 128] {
    let mut lut = [0.0; 128];
    let mut i = 0;
    while i < 128 {
        let x = i as f32 / 127.0;
        // Approximate x^2.5 using x * x * sqrt(x)
        // sqrt(x) approximation using one iteration of Newton's method
        let x2 = x * x;
        let sqrt_x = 0.5 * (x + 1.0); // Simple approximation of sqrt(x)
        lut[i] = f32::min(x2 * sqrt_x + 0.03, 1.0);
        i += 1;
    }
    lut
}

/// Returns the size of a `Voice` in bytes.
pub fn get_voice_size_byte() -> usize {
    std::mem::size_of::<Voice>()
}

/// Calculates the total memory usage for a given number of voices.
///
/// This function multiplies the size of a single `Voice` by the number of voices to calculate the total memory usage.
///
/// # Parameters
///
/// * `voice_count`: The number of voices.
///
/// # Returns
///
/// The total memory usage in bytes.
pub fn calculate_voice_memory_usage(voice_count: usize) -> usize {
    let voice_memory_usage = get_voice_size_byte() * voice_count;

    voice_memory_usage
}

#[derive(Clone, Copy, Debug)]
pub enum Channel {
    Mono = 1,
    Stereo,
}

macro_rules! impl_channel_from {
    ($($t:ty),*) => {
        $(
            impl From<Channel> for $t {
                fn from(channel: Channel) -> Self {
                    channel as u8 as $t
                }
            }
        )*
    };
}

macro_rules! impl_channel_try_from {
    ($($t:ty),*) => {
        $(
            impl TryFrom<$t> for Channel {
                type Error = &'static str;

                fn try_from(value: $t) -> Result<Self, Self::Error> {
                    match value {
                        1 => Ok(Channel::Mono),
                        2 => Ok(Channel::Stereo),
                        _ => Err("Invalid value for Channel"),
                    }
                }
            }
        )*
    };
}

impl_channel_from!(u8, u16, u32, u64, u128, usize);
impl_channel_from!(i8, i16, i32, i64, i128, isize);
impl_channel_from!(f32, f64);
impl_channel_from!(char);

impl_channel_try_from!(u8, u16, u32, u64, u128, usize);
impl_channel_try_from!(i8, i16, i32, i64, i128, isize);

pub struct KSynth {
    velocity_lut: [f32; 128],
    midi_queue: Vec<u32>,
    midi_channel: [MidiChannel; 16],
    sample_rate: u32,
    fade_out_duration: Duration,
    rendering_time: f32,
    samples: Arc<RwLock<HashMap<u8, Sample>>>,
    num_channel: Channel,
    voices: Vec<Voice>,
    polyphony: u32,
    max_polyphony: u32,
}

impl KSynth {
    pub fn new(
        sample_rate: u32,
        num_channel: Channel,
        max_polyphony: u32,
        fade_out_duration: Option<Duration>,
        samples: Arc<RwLock<HashMap<u8, Sample>>>,
    ) -> Self {
        let mut resampled_samples = HashMap::new();

        if let Ok(samples_guard) = samples.read() {
            for (&note, sample) in samples_guard.iter() {
                let resampled = sample.resample(sample_rate);
                resampled_samples.insert(note, resampled);
            }
        }

        let new_samples = Arc::new(RwLock::new(resampled_samples));

        let synth = Self {
            velocity_lut: precompute_velocity_lut(),
            midi_queue: Vec::new(),
            midi_channel: [MidiChannel::default(); 16],
            sample_rate,
            fade_out_duration: fade_out_duration.unwrap_or(FADE_OUT_DURATION),
            rendering_time: 0.0,
            samples: new_samples,
            num_channel,
            voices: Vec::with_capacity(max_polyphony.max(1) as usize),
            polyphony: 0,
            max_polyphony: max_polyphony.max(1).min(MAX_POLYPHONY),
        };

        synth
    }

    pub fn set_samples(&mut self, samples: Arc<RwLock<HashMap<u8, Sample>>>) {
        // Stop all sound
        for voice in self.voices.iter_mut() {
            voice.set_is_active(false);
        }

        // Remove inactive voice from voices array
        self.voices.retain(|v| v.get_is_active());

        let mut resampled_samples = HashMap::new();

        if let Ok(samples_guard) = samples.read() {
            for (&note, sample) in samples_guard.iter() {
                let resampled = sample.resample(self.sample_rate);
                resampled_samples.insert(note, resampled);
            }
        }

        let new_samples = Arc::new(RwLock::new(resampled_samples));
        self.samples = new_samples;
    }

    pub fn queue_midi_cmd(&mut self, cmd: u32) {
        self.midi_queue.push(cmd);
    }

    pub fn get_fade_out_duration(&self) -> Duration {
        self.fade_out_duration
    }

    pub fn set_fade_out_duration(&mut self, fade_out_duration: Duration) {
        self.fade_out_duration = fade_out_duration;
    }

    pub fn reset_fade_out_duration(&mut self) {
        self.fade_out_duration = FADE_OUT_DURATION;
    }

    pub fn get_rendering_time(&self) -> f32 {
        self.rendering_time
    }

    pub fn get_polyphony(&self) -> u32 {
        self.polyphony
    }

    pub fn get_max_polyphony(&self) -> u32 {
        self.max_polyphony
    }

    pub fn set_max_polyphony(&mut self, max_polyphony: u32) {
        if max_polyphony == 0 {
            return;
        }

        // Stop all sound
        for voice in self.voices.iter_mut() {
            voice.set_is_active(false);
        }

        // Remove inactive voice from voices array
        self.voices.retain(|v| v.get_is_active());

        // Update max polyphony
        self.max_polyphony = max_polyphony.min(MAX_POLYPHONY);

        // Reset current polyphony
        self.polyphony = self.voices.len() as u32;
    }

    pub fn fill_buffer(&mut self, buffer: &mut [f32]) -> bool {
        let buffer_size = buffer.len();

        let num_channel = match self.num_channel {
            Channel::Mono => 1,
            Channel::Stereo => 2,
        };

        let frame_count = buffer_size / num_channel;

        if frame_count == 0 {
            return false;
        }

        let midi_cmds = std::mem::take(&mut self.midi_queue);
        for cmd in midi_cmds {
            let status = (cmd & 0xFF) as u8;
            let data1 = ((cmd >> 8) & 0xFF) as u8;
            let data2 = ((cmd >> 16) & 0xFF) as u8;

            let channel = (status & 0x0F) as u8;

            match status & 0xF0 {
                // Note On
                0x90 => {
                    if data2 > 0 {
                        self.note_on(channel, data1, data2);
                    } else {
                        self.note_off(channel, data1);
                    }
                }
                // Note Off
                0x80 => {
                    self.note_off(channel, data1);
                }
                // Control Change
                0xB0 => {
                    match data1 {
                        // Pan
                        0x0A => {
                            let pan = (data2 as f32 / 127.0) * 2.0 - 1.0;
                            self.midi_channel[channel as usize].set_pan(pan);
                        }
                        // Damper pedal
                        0x40 => {
                            let is_sustain = data2 > 63;
                            self.midi_channel[channel as usize].set_sustain(is_sustain);

                            if !is_sustain {
                                for voice in self.voices.iter_mut() {
                                    if voice.get_channel() == channel
                                        && !voice.get_is_key_down()
                                        && !voice.get_is_releasing()
                                    {
                                        voice.set_is_releasing(true);
                                    }
                                }
                            }
                        }
                        // RPN MSB: CC101 = 0
                        0x65 => {
                            // CC101 (RPN MSB)
                            if data2 == 0 {
                                self.midi_channel[channel as usize].set_rpn_msb(0);
                            }
                        }
                        // RPN LSB: CC100 = 0
                        0x64 => {
                            // CC100 (RPN LSB)
                            if data2 == 0 {
                                self.midi_channel[channel as usize].set_rpn_lsb(0);
                            }
                        }
                        // Data Entry MSB: CC6 (Pitch Bend Sensitivity)
                        0x06 => {
                            // CC6 (Data Entry)
                            if self.midi_channel[channel as usize].get_rpn_msb() == 0
                                && self.midi_channel[channel as usize].get_rpn_lsb() == 0
                            {
                                // Set pitch bend range in semitones
                                let bend_range = data2 as u8;
                                self.midi_channel[channel as usize]
                                    .set_bend_range_semitone(bend_range);
                            }
                        }
                        _ => {}
                    }
                }
                0xE0 => {
                    // Reconstruct 14bit pitch bend value (0-16383) (Value = 128 * MSB + LSB)
                    let raw_value = ((data1 as u16) & 0x7F) | (((data2 as u16) & 0x7F) << 7);
                    let pitch_bend = raw_value as i16 - 8192;

                    // Proper normalization to [-1.0, +1.0]
                    let normalized = pitch_bend as f32 / 8192.0;

                    // Apply bend range in semitones
                    let bend_range =
                        self.midi_channel[channel as usize].get_bend_range_semitone() as f32;
                    let semitones = normalized * bend_range;

                    let pitch_factor = 2.0f32.powf(semitones / 12.0);

                    self.midi_channel[channel as usize].set_pitch_factor(pitch_factor);
                }
                _ => {}
            }
        }

        let samples_guard = self.samples.read().unwrap();

        let rendering_time_start = Instant::now();

        for frame in 0..frame_count {
            let buffer_index = frame * num_channel;

            // Process active voices
            for voice in self.voices.iter_mut().filter(|v| v.get_is_active()) {
                if let Some(sample) = samples_guard.get(&voice.get_note()) {
                    let sample_data = sample.get_sample_data();
                    let sample_length = sample.sample_length();
                    let sample_loop = sample.get_sample_loop();

                    // Fade out processing
                    let mut amplitude = 1.0;
                    if voice.get_is_releasing() {
                        if let Some(release_start) = voice.get_release_start_index() {
                            let samples_since_release =
                                voice.current_sample_index() - release_start;
                            let fade_samples = (self.sample_rate as f32
                                * self.fade_out_duration.as_secs_f32())
                                as f32;

                            if samples_since_release >= fade_samples {
                                voice.set_is_active(false);
                            } else {
                                amplitude *=
                                    1.0 - (samples_since_release as f32 / fade_samples as f32);
                            }
                        }
                    }

                    let vel = voice.get_velocity() as f32;
                    let velocity_factor = self.velocity_lut[vel as usize];
                    amplitude *= velocity_factor;

                    let pitch_factor =
                        self.midi_channel[voice.get_channel() as usize].get_pitch_factor();

                    // Sample processing
                    let sample_data_len = match sample_data {
                        SampleData::Mono(data) => data.len(),
                        SampleData::Stereo(data) => data.len(),
                    };

                    if sample_data_len > 1 {
                        let sample_index_f = voice.current_sample_index();
                        let sample_index = sample_index_f.floor() as usize;
                        let next_index = (sample_index + 1).min(sample_data_len - 1);
                        let frac = sample_index_f - sample_index as f32;

                        let (left, right) = match sample_data {
                            SampleData::Mono(data) => {
                                let s1 = data.get(sample_index).copied().unwrap_or(0) as f32;
                                let s2 = data.get(next_index).copied().unwrap_or(0) as f32;
                                let value = s1 + (s2 - s1) * frac;
                                let val = value / i16::MAX as f32;
                                (val, val)
                            }
                            SampleData::Stereo(data) => {
                                let (l1, r1) = data.get(sample_index).copied().unwrap_or((0, 0));
                                let (l2, r2) = data.get(next_index).copied().unwrap_or((0, 0));
                                let left = l1 as f32 + (l2 as f32 - l1 as f32) * frac;
                                let right = r1 as f32 + (r2 as f32 - r1 as f32) * frac;
                                (left / i16::MAX as f32, right / i16::MAX as f32)
                            }
                        };

                        // Pan handling (stereo)
                        let pan = self.midi_channel[voice.get_channel() as usize].get_pan();
                        let left_pan = ((1.0 - pan) * 0.5).sqrt();
                        let right_pan = ((1.0 + pan) * 0.5).sqrt();

                        match self.num_channel {
                            Channel::Mono => {
                                buffer[buffer_index] += (left + right) * 0.5 * amplitude;
                            }
                            Channel::Stereo => {
                                buffer[buffer_index] += left * amplitude * left_pan;
                                buffer[buffer_index + 1] += right * amplitude * right_pan;
                            }
                        }

                        // Advance sample index with pitch factor
                        let sample_playback_rate =
                            sample.get_sample_rate() as f32 / self.sample_rate as f32;
                        voice.increment_sample_index(pitch_factor * sample_playback_rate);

                        // Loop or deactivate
                        if let Some(loop_info) = sample_loop {
                            if voice.current_sample_index() >= loop_info.end() as f32 {
                                voice.set_current_sample_index(loop_info.start() as f32);
                            }
                        } else {
                            if voice.current_sample_index() >= sample_length as f32 {
                                voice.set_is_active(false);
                            }
                        }
                    }
                }
            }
        }

        let rendering_time_end = Instant::now();
        let elapsed_time = rendering_time_end.duration_since(rendering_time_start);
        let elapsed_time_ms = elapsed_time.as_secs_f32() * 1e3;
        let rendering_time = elapsed_time_ms / buffer_size as f32;
        self.rendering_time = rendering_time * 100.0;

        // Remove inactive voices
        self.voices.retain(|v| v.get_is_active());
        self.polyphony = self.voices.len() as u32;

        true
    }

    fn note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        if channel > 15 || note > 127 || velocity > 127 || channel == 9 {
            return;
        }

        if self.polyphony >= self.max_polyphony {
            if let Some(quietest_voice_index) = self
                .voices
                .iter()
                .enumerate()
                .min_by_key(|(_, v)| v.get_velocity())
                .map(|(index, _)| index)
            {
                self.voices.remove(quietest_voice_index);
                self.polyphony -= 1;
            }
        }

        if velocity == 0 {
            self.note_off(channel, note);
            return;
        }

        let voice = Voice::new(channel, note, velocity);
        self.voices.push(voice);
        self.polyphony += 1;
    }

    fn note_off(&mut self, channel: u8, note: u8) {
        if channel > 15 || note > 127 || channel == 9 {
            return;
        }

        for voice in self.voices.iter_mut() {
            if voice.get_channel() == channel && voice.get_note() == note {
                voice.set_is_key_down(false);

                if !self.midi_channel[channel as usize].get_sustain() {
                    voice.set_is_releasing(true);
                }
            }
        }
    }
}
