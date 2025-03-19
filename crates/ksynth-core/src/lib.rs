pub mod sample;
pub mod voice;

use std::{collections::HashMap, time::Instant};

use sample::Sample;
use voice::Voice;

pub const MAX_POLYPHONY: u32 = 4 * 1024 * 1024 * (1024 / std::mem::size_of::<Voice>() as u32);

const FADE_OUT_DURATION: f32 = 0.1;

#[derive(Clone, Debug)]
pub enum Instrument {
    Piano,
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

pub struct KSynth {
    midi_queue: Vec<u32>,
    sample_rate: u32,
    rendering_time: f32,
    samples: HashMap<u8, Sample>,
    channels: Channel,
    instrument: Instrument,
    voices: Vec<voice::Voice>,
    polyphony: usize,
    max_polyphony: usize,
}

#[derive(Clone)]
pub enum Channel {
    Mono,
    Stereo,
}

impl KSynth {
    fn calculate_sample(&self, time: f32, frequency: f32) -> f32 {
        match self.instrument {
            Instrument::Piano => {
                let w = 2.0 * std::f32::consts::PI * frequency;

                let mut y = 0.6 * (1.0 * w * time).sin() * (-0.0015 * w * time).exp();
                y += 0.4 * (2.0 * w * time).sin() * (-0.0015 * w * time).exp();
                y += y * y * y;

                let volume_scale: f32 = 0.5;

                y * volume_scale.powf(2.0)
            }
        }
    }

    fn precalculate_sample(&mut self) {
        for key in 0..128 {
            // Frequency calculation (440Hz * 2^((key-69)/12))
            let frequency = 440.0 * 2.0_f32.powf((key as f32 - 69.0) / 12.0);

            // Calculate sample (5 seconds)
            let sample_length = (self.sample_rate * 5) as usize;

            // Generate waveform
            let mut wave_data = Vec::with_capacity(sample_length);
            for i in 0..sample_length {
                let t = i as f32 / self.sample_rate as f32;
                let sample = self.calculate_sample(t, frequency);

                let scaled_sample = (sample * i16::MAX as f32).round() as i16;
                wave_data.push(scaled_sample);
            }

            // Add sample to HashMap
            self.samples.insert(key as u8, Sample::new(wave_data));
        }
    }

    pub fn new(sample_rate: u32, channels: Channel, max_polyphony: u32) -> Self {
        let mut synth = Self {
            midi_queue: Vec::new(),
            sample_rate,
            rendering_time: 0.0,
            samples: HashMap::new(),
            channels,
            instrument: Instrument::Piano,
            voices: Vec::with_capacity(max_polyphony as usize),
            polyphony: 0,
            max_polyphony: max_polyphony.min(MAX_POLYPHONY) as usize,
        };

        synth.precalculate_sample();

        synth
    }

    pub fn queue_midi_cmd(&mut self, cmd: u32) {
        self.midi_queue.push(cmd);
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
        let min_size = match self.channels {
            Channel::Mono => 1,
            Channel::Stereo => 2,
        };

        if buffer_size < min_size {
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
                        // Call note off command always when note off command is sent
                        self.note_off(channel, note);
                    }
                }
                0x80 => {
                    // Call note off command always when note off command is sent
                    self.note_off(channel, note);
                }
                _ => {}
            }
        }

        let rendering_time_start = Instant::now();

        // Process active voice
        for voice in self.voices.iter_mut().filter(|v| v.get_is_active()) {
            if let Some(sample) = self.samples.get(&voice.get_note()) {
                let samples = &sample.sample_data;
                let vel = voice.get_velocity() as f32 / 127.0;
                let velocity_factor = (vel.powf(2.5) + 0.03).min(1.0);

                // Process each sample
                let mut i = 0;
                while i < buffer_size {
                    let sample_index = voice.current_sample_index();

                    // If the sample index exceeds the sample range (when the sample has ended)
                    if sample_index >= samples.len() {
                        voice.set_is_active(false);
                        break;
                    } else {
                        // Get sample
                        let sample_value = samples[sample_index] as f32 / i16::MAX as f32;

                        // Fade-out
                        let fade_factor = if voice.get_is_releasing() {
                            let fade_out_samples =
                                (FADE_OUT_DURATION * self.sample_rate as f32) as usize;
                            let elapsed = sample_index as f32;
                            let fade = 1.0 - (elapsed / fade_out_samples as f32);

                            fade.max(0.0)
                        } else {
                            1.0
                        };

                        let final_sample = sample_value * velocity_factor * fade_factor;

                        match self.channels {
                            Channel::Stereo => {
                                let buffer_index = i * 2;
                                if buffer_index + 1 < buffer.len() {
                                    buffer[buffer_index] += final_sample;
                                    buffer[buffer_index + 1] += final_sample;
                                }
                            }
                            Channel::Mono => {
                                if i < buffer.len() {
                                    buffer[i] += final_sample;
                                }
                            }
                        }

                        voice.increment_sample_index();

                        // If the fade-out is complete, stop the voice
                        if voice.get_is_releasing() && fade_factor <= 0.01 {
                            voice.set_is_active(false);
                        }
                    }

                    i += 1;
                }
            }
        }

        let rendering_time_end = Instant::now();
        let elapsed_time = rendering_time_end.duration_since(rendering_time_start);
        let elapsed_time_ms = elapsed_time.as_secs_f32() * 1e3;
        let rendering_time = elapsed_time_ms / buffer_size as f32;
        self.rendering_time = rendering_time * 100.0;

        self.voices.retain(|v| v.get_is_active());
        self.polyphony = self.voices.len();
    }

    fn note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        let voice = Voice::new(channel, note, velocity);
        self.voices.push(voice);

        if self.voices.len() > self.max_polyphony as usize {
            self.voices.remove(0);
        }
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
