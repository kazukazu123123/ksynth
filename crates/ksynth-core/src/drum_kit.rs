use crate::Channel;
use crate::midi_channel::MidiChannel;
use crate::sample::Sample;
use crate::sample::SampleData;
use crate::voice::Voice;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(Clone)]
pub struct DrumKit {
    drum_samples: Arc<RwLock<HashMap<u8, Sample>>>,
    drum_voices: Vec<Voice>,
}

impl DrumKit {
    pub fn new(drum_samples: HashMap<u8, Sample>) -> Self {
        Self {
            drum_samples: Arc::new(RwLock::new(drum_samples)),
            drum_voices: Vec::new(),
        }
    }

    pub fn add_sample(&mut self, key: u8, sample: Sample) {
        self.drum_samples.write().unwrap().insert(key, sample);
    }

    pub fn resample_all_drums(&self, sample_rate: u32) {
        let mut drum_samples_guard = self.drum_samples.write().unwrap();
        let mut resampled_map = HashMap::new();
        for (&note, sample) in drum_samples_guard.iter() {
            let resampled = sample.resample(sample_rate);
            resampled_map.insert(note, resampled);
        }
        *drum_samples_guard = resampled_map;
    }

    pub fn note_on_drum(&mut self, note: u8, velocity: u8) {
        let drum_samples_guard = self.drum_samples.read().unwrap();

        // Define hi-hat notes and their choke group ID
        const HIHAT_CHOKE_GROUP_ID: u8 = 1;
        const HIHAT_NOTES: [u8; 3] = [42, 44, 46]; // Closed, Pedal, Open

        let mut choke_existing_hihats = false;
        let mut new_voice_choke_group_id: Option<u8> = None;

        if HIHAT_NOTES.contains(&note) {
            choke_existing_hihats = true;
            new_voice_choke_group_id = Some(HIHAT_CHOKE_GROUP_ID);
        }

        // Choke any existing voices in the same choke group
        if choke_existing_hihats {
            for voice in self.drum_voices.iter_mut() {
                if voice.get_choke_group_id() == new_voice_choke_group_id && voice.get_is_active() {
                    voice.set_is_releasing(true);
                }
            }
        }

        if drum_samples_guard.contains_key(&note) {
            let voice = Voice::new(9, note, velocity, new_voice_choke_group_id); // Pass choke_group_id
            self.drum_voices.push(voice);
        }
    }

    pub fn note_off_drum(&mut self, note: u8) {
        for voice in self.drum_voices.iter_mut() {
            if voice.get_note() == note && voice.get_is_key_down() {
                voice.set_is_key_down(false);
                voice.set_is_releasing(true);
            }
        }
    }

    pub fn process_drum_voices(
        &mut self,
        buffer: &mut [f32],
        buffer_index: usize,
        fade_frames: u64,
        velocity_lut: &[f32; 128],
        midi_channel_states: &[MidiChannel; 16],
        ksynth_num_channel: Channel,
        ksynth_sample_rate: u32,
    ) {
        let drum_samples_guard = self.drum_samples.read().unwrap();

        let fade_reciprocal = if fade_frames > 0 {
            1.0 / fade_frames as f32
        } else {
            0.0
        };

        for voice in self.drum_voices.iter_mut().filter(|v| v.get_is_active()) {
            let voice_releasing = voice.get_is_releasing();

            if let Some(sample) = drum_samples_guard.get(&voice.get_note()) {
                let sample_data = sample.get_sample_data();
                let sample_length = sample.sample_length();
                let sample_loop = sample.get_sample_loop();

                if sample_length == 0 {
                    voice.set_is_active(false);
                    continue;
                }

                // Fade out processing
                let mut amplitude = 1.0;
                if voice.get_is_releasing() {
                    if let Some(frames_since_release) = voice.get_frames_since_release() {
                        let adjusted_fade_frames = fade_frames;

                        if frames_since_release >= adjusted_fade_frames {
                            voice.set_is_active(false);
                            continue;
                        } else if adjusted_fade_frames > 0 {
                            amplitude *= 1.0 - (frames_since_release as f32 * fade_reciprocal);
                        } else {
                            amplitude = 0.0;
                        }
                    }
                }

                let vel = voice.get_velocity() as f32;
                let velocity_factor = velocity_lut[vel as usize];
                let volume_factor =
                    midi_channel_states[voice.get_channel() as usize].get_volume_factor();

                amplitude *= velocity_factor * volume_factor;

                let pitch_factor =
                    midi_channel_states[voice.get_channel() as usize].get_pitch_factor();

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
                            let val = value * (1.0 / i16::MAX as f32);
                            (val, val)
                        }
                        SampleData::Stereo(data) => {
                            let (l1, r1) = data.get(sample_index).copied().unwrap_or((0, 0));
                            let (l2, r2) = data.get(next_index).copied().unwrap_or((0, 0));
                            let left = l1 as f32 + (l2 as f32 - l1 as f32) * frac;
                            let right = r1 as f32 + (r2 as f32 - r1 as f32) * frac;
                            (
                                left * (1.0 / i16::MAX as f32),
                                right * (1.0 / i16::MAX as f32),
                            )
                        }
                    };

                    // Pan handling (stereo)
                    let left_pan = midi_channel_states[voice.get_channel() as usize].get_pan_l();
                    let right_pan = midi_channel_states[voice.get_channel() as usize].get_pan_r();

                    match ksynth_num_channel {
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
                        sample.get_sample_rate() as f32 / ksynth_sample_rate as f32;
                    voice.increment_sample_index(pitch_factor * sample_playback_rate);

                    // Loop or deactivate
                    let mut reached_end = false;
                    if sample_loop.is_none() {
                        // Drums usually don't loop
                        if voice.current_sample_index() >= sample_length as f32 {
                            reached_end = true;
                        }
                    } else {
                        // Existing loop logic from KSynth::fill_buffer
                        if voice.current_sample_index() >= sample_loop.unwrap().end() as f32 {
                            voice.set_current_sample_index(
                                sample_loop.unwrap().start() as f32
                                    + (voice.current_sample_index()
                                        - sample_loop.unwrap().end() as f32),
                            );
                        }
                    }

                    if reached_end {
                        voice.set_is_active(false);
                    }

                    if voice_releasing {
                        voice.increment_frames_since_release();
                    }
                } else if sample_data_len <= 1 {
                    voice.set_is_active(false);
                }
            } else {
                voice.set_is_active(false);
            }
        }
    }

    pub fn clean_up_inactive_voices(&mut self) {
        self.drum_voices.retain(|v| v.get_is_active());
    }

    pub fn get_drum_voices(&self) -> &Vec<Voice> {
        &self.drum_voices
    }

    pub fn get_drum_voices_mut(&mut self) -> &mut Vec<Voice> {
        &mut self.drum_voices
    }
}
