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

const FADE_OUT_DURATION: Duration = Duration::from_millis(100);

const fn precompute_velocity_lut() -> [f32; 128] {
    let mut lut = [0.0; 128];
    let mut i = 0;
    while i < 128 {
        let x = i as f32 / 127.0;
        let x2 = x * x;
        let x3 = x2 * x;

        lut[i] = (0.6 * x2 + 0.4 * x3 + 0.03).min(1.0);
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
    Mono,
    Stereo,
}

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
    polyphony: usize,
    max_polyphony: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum SampleMode {
    Mono,
    Stereo,
}

impl KSynth {
    pub fn new(
        sample_rate: u32,
        num_channel: Channel,
        max_polyphony: u32,
        fade_out_duration: Option<Duration>,
        samples: Arc<RwLock<HashMap<u8, Sample>>>,
    ) -> Self {
        let mut synth = Self {
            velocity_lut: precompute_velocity_lut(),
            midi_queue: Vec::new(),
            midi_channel: [MidiChannel::default(); 16],
            sample_rate,
            fade_out_duration: fade_out_duration.unwrap_or(FADE_OUT_DURATION),
            rendering_time: 0.0,
            samples,
            num_channel,
            voices: Vec::with_capacity(max_polyphony as usize),
            polyphony: 0,
            max_polyphony: max_polyphony.min(MAX_POLYPHONY) as usize,
        };

        for i in 0..128 {
            synth.velocity_lut[i] = f32::min((i as f32 / 127.0).powf(2.5) + 0.03, 1.0);
        }

        synth
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
        self.polyphony as u32
    }

    pub fn get_max_polyphony(&self) -> u32 {
        self.max_polyphony as u32
    }

    pub fn set_samples(&mut self, samples: Arc<RwLock<HashMap<u8, Sample>>>) {
        // Stop all sound
        for voice in self.voices.iter_mut() {
            voice.set_is_active(false);
        }

        // Remove inactive voice from voices array
        self.voices.retain(|v| v.get_is_active());

        self.samples = samples;
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
        self.polyphony = self.voices.len();
    }

    pub fn fill_buffer(&mut self, buffer: &mut [f32]) -> bool {
        let buffer_size = buffer.len();

        let channel = match self.num_channel {
            Channel::Mono => 1,
            Channel::Stereo => 2,
        };

        let frame_count = buffer_size / channel;

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
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        let samples_guard = self.samples.read().unwrap();

        let rendering_time_start = Instant::now();

        for frame in 0..frame_count {
            let buffer_index = frame * channel;

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
                                as usize;

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

                    // Sample processing
                    let sample_data_len = match sample_data {
                        SampleData::Mono(data) => data.len(),
                        SampleData::Stereo(data) => data.len(),
                    };

                    if sample_data_len != 0 {
                        let sample_index = voice.current_sample_index() % sample_data_len;
                        let (left, right) = match sample_data {
                            SampleData::Mono(data) => {
                                let value = data[sample_index] as f32 / i16::MAX as f32;
                                (value, value)
                            }
                            SampleData::Stereo(data) => {
                                let (left, right) = data[sample_index];
                                (
                                    left as f32 / i16::MAX as f32,
                                    right as f32 / i16::MAX as f32,
                                )
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

        true
    }

    fn note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        if channel > 15 || note > 127 || velocity > 127 {
            return;
        }

        if channel == 9 {
            return;
        }

        if velocity == 0 {
            self.note_off(channel, note);
            return;
        }

        if self.polyphony >= self.max_polyphony {
            if let Some(quietest_voice_index) = self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| v.get_is_active())
                .min_by_key(|(_, v)| v.get_velocity())
                .map(|(index, _)| index)
            {
                self.voices.remove(quietest_voice_index);
                self.polyphony -= 1;
            }
        }

        let voice = Voice::new(channel, note, velocity);
        self.voices.push(voice);
        self.polyphony += 1;
    }

    fn note_off(&mut self, channel: u8, note: u8) {
        if channel > 15 || note > 127 {
            return;
        }

        if channel == 9 {
            return;
        }

        for voice in self.voices.iter_mut() {
            if voice.get_channel() == channel && voice.get_note() == note {
                voice.set_is_releasing(true);
            }
        }
    }
}
