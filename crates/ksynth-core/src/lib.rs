pub mod sample;
pub mod voice;

use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};

use sample::{Sample, SampleData};
use voice::Voice;

pub const MAX_POLYPHONY: u32 = 4 * 1024 * 1024 * (1024 / std::mem::size_of::<Voice>() as u32);

const FADE_IN_DURATION: f32 = 0.01;
const FADE_OUT_DURATION: f32 = 0.01;

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
    Mono,
    Stereo,
}

pub struct KSynth {
    midi_queue: Vec<u32>,
    sample_rate: u32,
    fade_in_duration: f32,
    fade_out_duration: f32,
    rendering_time: f32,
    samples: HashMap<u8, Sample>,
    channels: Channel,
    voices: VecDeque<voice::Voice>,
    polyphony: usize,
    max_polyphony: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum SampleMode {
    Mono,
    Stereo,
}

impl KSynth {
    fn calculate_sample(&self, time: f32, frequency: f32) -> f32 {
        let w = 2.0 * std::f32::consts::PI * frequency;

        let mut y = 0.6 * (1.0 * w * time).sin() * (-0.0015 * w * time).exp();
        y += 0.4 * (2.0 * w * time).sin() * (-0.0015 * w * time).exp();
        y += y * y * y;

        let volume_scale: f32 = 0.5;

        y * volume_scale.powf(2.0)
    }

    fn calculate_sample_stereo(&self, time: f32, frequency: f32) -> (f32, f32) {
        let sample = self.calculate_sample(time, frequency);

        (sample, sample)
    }

    fn precalculate_sample(&mut self) {
        let sample_length = (self.sample_rate * 5) as usize;
        for key in 0..128 {
            let frequency = 440.0 * 2.0_f32.powf((key as f32 - 69.0) / 12.0);

            let wave_data: Vec<_> = (0..sample_length)
                .map(|i| {
                    let t = i as f32 / self.sample_rate as f32;
                    let (sample_left, sample_right) = self.calculate_sample_stereo(t, frequency);
                    let scaled_sample_left = (sample_left * i16::MAX as f32).round() as i16;
                    let scaled_sample_right = (sample_right * i16::MAX as f32).round() as i16;
                    (scaled_sample_left, scaled_sample_right)
                })
                .collect();

            let sample_data = SampleData::Stereo(wave_data);

            self.samples
                .insert(key as u8, Sample::new(self.sample_rate, sample_data, None));
        }
    }

    pub fn new(
        sample_rate: u32,
        channels: Channel,
        max_polyphony: u32,
        fade_in_duration: Option<f32>,
        fade_out_duration: Option<f32>,
    ) -> Self {
        let mut synth = Self {
            midi_queue: Vec::new(),
            sample_rate,
            fade_in_duration: fade_in_duration.unwrap_or(FADE_IN_DURATION),
            fade_out_duration: fade_out_duration.unwrap_or(FADE_OUT_DURATION),
            rendering_time: 0.0,
            samples: HashMap::new(),
            channels,
            voices: VecDeque::with_capacity(max_polyphony as usize),
            polyphony: 0,
            max_polyphony: max_polyphony.min(MAX_POLYPHONY) as usize,
        };

        synth.precalculate_sample();

        synth
    }

    pub fn queue_midi_cmd(&mut self, cmd: u32) {
        self.midi_queue.push(cmd);
    }

    pub fn get_fade_in_duration(&self) -> f32 {
        self.fade_in_duration
    }

    pub fn get_fade_out_duration(&self) -> f32 {
        self.fade_out_duration
    }

    pub fn set_fade_in_duration(&mut self, fade_in_duration: f32) {
        self.fade_in_duration = fade_in_duration;
    }

    pub fn set_fade_out_duration(&mut self, fade_out_duration: f32) {
        self.fade_out_duration = fade_out_duration;
    }

    pub fn reset_fade_in_duration(&mut self) {
        self.fade_in_duration = FADE_IN_DURATION;
    }

    pub fn reset_fade_out_duration(&mut self) {
        self.fade_out_duration = FADE_OUT_DURATION;
    }

    pub fn get_rendering_time(&self) -> f32 {
        self.rendering_time
    }

    pub fn get_polyphony(&self) -> u32 {
        self.polyphony as u32
    }

    pub fn get_max_polyphony(&self) -> u32 {
        self.max_polyphony as u32
    }

    pub fn set_max_polyphony(&mut self, max_polyphony: u32) {
        // Stop all sound
        for voice in self.voices.iter_mut() {
            voice.set_is_active(false);
        }

        // Remove inactive voice from voices array
        self.voices.retain(|v| v.get_is_active());

        // Update max polyphony
        self.max_polyphony = max_polyphony.min(MAX_POLYPHONY) as usize;

        // Reset current polyphony
        self.polyphony = 0;
    }

    pub fn fill_buffer(&mut self, buffer: &mut [f32], buffer_size: usize) {
        let channels = match self.channels {
            Channel::Mono => 1,
            Channel::Stereo => 2,
        };

        let frame_count = buffer_size / channels;

        if frame_count == 0 {
            return;
        }

        let midi_cmds = std::mem::take(&mut self.midi_queue);
        for cmd in midi_cmds {
            let status = (cmd & 0xFF) as u8;
            let note = ((cmd >> 8) & 0xFF) as u8;
            let velocity = ((cmd >> 16) & 0xFF) as u8;

            let channel = (status & 0x0F) as u8;

            match status & 0xF0 {
                0x90 => {
                    if velocity > 0 {
                        self.note_on(channel, note, velocity);
                    } else {
                        self.note_off(channel, note);
                    }
                }
                0x80 => {
                    self.note_off(channel, note);
                }
                _ => {}
            }
        }

        let rendering_time_start = Instant::now();

        for frame in 0..frame_count {
            let buffer_index = frame * channels;

            // Process active voices
            for voice in self.voices.iter_mut().filter(|v| v.get_is_active()) {
                if voice.get_channel() == 9 {
                    continue;
                }

                if let Some(sample) = self.samples.get(&voice.get_note()) {
                    let sample_data = sample.get_sample_data();
                    let sample_length = sample.sample_length();
                    let sample_loop = sample.get_sample_loop();

                    // Fade out processing
                    let mut amplitude = 1.0;
                    if voice.get_is_releasing() {
                        if let Some(release_start) = voice.get_release_start_index() {
                            let samples_since_release =
                                voice.current_sample_index() - release_start;
                            let fade_samples =
                                (self.sample_rate as f32 * self.fade_out_duration) as usize;

                            // Fade out calculation
                            amplitude *=
                                1.0 - (samples_since_release as f32 / fade_samples as f32).min(1.0);

                            if samples_since_release >= fade_samples {
                                voice.set_is_active(false);
                            }
                        }
                    }

                    // Fade in processing
                    if !voice.get_is_releasing() {
                        let fade_in_samples =
                            (self.sample_rate as f32 * self.fade_in_duration) as usize;
                        let samples_since_start = voice.current_sample_index();
                        if samples_since_start < fade_in_samples {
                            amplitude = samples_since_start as f32 / fade_in_samples as f32;
                        }
                    }

                    let vel = voice.get_velocity() as f32;
                    let log_vel = vel / 127.0;
                    let velocity_factor = f32::min(log_vel.powf(2.5) + 0.03, 1.0);
                    amplitude *= velocity_factor;

                    // Sample processing
                    match (self.channels, sample_data) {
                        (Channel::Mono, SampleData::Mono(data)) => {
                            if !data.is_empty() {
                                let sample_index = voice.current_sample_index() % data.len();
                                let sample_value =
                                    data[sample_index] as f32 / i16::MAX as f32 * amplitude;
                                buffer[buffer_index] += sample_value;
                            }
                        }
                        (Channel::Mono, SampleData::Stereo(data)) => {
                            if !data.is_empty() {
                                let sample_index = voice.current_sample_index() % data.len();
                                let (left, right) = data[sample_index];
                                let sample_value =
                                    ((left + right) / 2) as f32 / i16::MAX as f32 * amplitude;
                                buffer[buffer_index] += sample_value;
                            }
                        }
                        (Channel::Stereo, SampleData::Mono(data)) => {
                            if !data.is_empty() {
                                let sample_index = voice.current_sample_index() % data.len();
                                let sample_value =
                                    data[sample_index] as f32 / i16::MAX as f32 * amplitude;
                                buffer[buffer_index] += sample_value;
                                buffer[buffer_index + 1] += sample_value;
                            }
                        }
                        (Channel::Stereo, SampleData::Stereo(data)) => {
                            if !data.is_empty() {
                                let sample_index = voice.current_sample_index() % data.len();
                                let (left, right) = data[sample_index];
                                let left_value = left as f32 / i16::MAX as f32 * amplitude;
                                let right_value = right as f32 / i16::MAX as f32 * amplitude;
                                buffer[buffer_index] += left_value;
                                buffer[buffer_index + 1] += right_value;
                            }
                        }
                    }

                    // Increment sample index
                    voice.increment_sample_index();

                    // Loop handling
                    if let Some(loop_info) = sample_loop {
                        if voice.current_sample_index() >= loop_info.end() {
                            voice.set_current_sample_index(loop_info.start());
                        }
                    } else {
                        // If sample index is greater than or equal to sample length, deactivate voice
                        if voice.current_sample_index() >= sample_length {
                            voice.set_is_active(false);
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
        self.polyphony = self.voices.len();
    }

    fn note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        let voice = Voice::new(channel, note, velocity);
        if self.voices.len() >= self.max_polyphony as usize {
            self.voices.pop_back();
        }
        self.voices.push_front(voice);
    }

    fn note_off(&mut self, channel: u8, note: u8) {
        for voice in self.voices.iter_mut() {
            if voice.get_channel() == channel && voice.get_note() == note {
                voice.set_is_releasing(true);
            }
        }
    }

    pub fn get_voice_size_byte() -> usize {
        std::mem::size_of::<Voice>()
    }
}
