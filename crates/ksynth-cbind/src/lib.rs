use ksynth_core::sample::{Sample, SampleData, SampleLoop};
use ksynth_core::{Channel, KSynth, drum_kit::DrumKit};
use std::collections::HashMap;
use std::os::raw::c_char;
use std::ptr;
use std::sync::{Arc, RwLock};

pub enum KSynthPtr {}
pub enum KSynthSampleMapPtr {}
pub enum KSynthDrumKitPtr {}

struct SampleMap {
    samples: HashMap<u8, Arc<Sample>>,
}

#[repr(C)]
pub struct KSynthSampleLoop {
    start: usize,
    end: usize,
}

/// Converts a given duration in milliseconds to the corresponding sample count based on the sample rate.
///
/// # Arguments
/// `sample_rate` - The sample rate of the audio in samples per second (Hz). This is typically a standard audio sample rate like 44,100 Hz or 48,000 Hz.
/// `duration_ms` - The duration of the audio or signal in milliseconds (ms). This represents how long the audio lasts.
///
/// # Returns
/// `u32` - The number of samples corresponding to the given duration in milliseconds, based on the specified sample rate.
#[unsafe(no_mangle)]
pub extern "C" fn ksynth_ms_to_sample(sample_rate: u32, duration_ms: u32) -> u32 {
    // The calculation is based on the formula described above.
    (sample_rate * duration_ms) / 1000
}

/// Returns the highest polyphony value that can be configured in ksynth.
///
/// # Returns
/// `u32` - Maximum configurable voice count.
#[unsafe(no_mangle)]
pub extern "C" fn ksynth_get_max_supported_polyphony() -> u32 {
    ksynth_core::MAX_POLYPHONY
}

/// Returns the git commit hash used at build time.
///
/// # Returns
/// `*const c_char` - Pointer to a C string (must NOT be freed by the caller) or null.
#[unsafe(no_mangle)]
pub extern "C" fn ksynth_get_git_commit_hash() -> *const c_char {
    ksynth_core::KSYNTH_BUILD_GIT_COMMIT_HASH.as_ptr() as *const c_char
}

/// Returns the memory size in bytes required for a single voice instance.
///
/// # Returns
/// The memory usage in bytes for one voice.
#[unsafe(no_mangle)]
pub extern "C" fn ksynth_get_voice_size_byte() -> usize {
    ksynth_core::ksynth_get_voice_size_byte()
}

/// Calculates the memory usage for a given number of voices.
///
/// # Arguments
/// `voice_count` - The number of voices to calculate memory usage for.
/// # Returns
/// The estimated memory usage in bytes for the specified number of voices.
#[unsafe(no_mangle)]
pub extern "C" fn ksynth_calculate_voice_memory_usage(voice_count: usize) -> usize {
    ksynth_core::ksynth_calculate_voice_memory_usage(voice_count)
}

/// Creates a new, empty sample map.
///
/// The returned pointer represents a shared sample map and must be freed
/// using `ksynth_sample_map_free` when no longer needed.
///
/// # Returns
/// A pointer to a new sample map instance, or `null` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn ksynth_sample_map_new() -> *mut KSynthSampleMapPtr {
    let sample_map = Box::new(SampleMap {
        samples: HashMap::new(),
    });
    Box::into_raw(sample_map) as *mut KSynthSampleMapPtr
}

/// Creates a new, empty drum kit.
///
/// The returned pointer represents a shared drum kit and must be freed
/// using `ksynth_drum_kit_free` when no longer needed.
///
/// # Returns
/// A pointer to a new drum kit instance, or `null` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn ksynth_drum_kit_new() -> *mut KSynthDrumKitPtr {
    let drum_kit = Box::new(DrumKit::new(HashMap::new()));
    Box::into_raw(drum_kit) as *mut KSynthDrumKitPtr
}

/// Adds or replaces a sample in the specified sample map.
///
/// This function copies the provided sample data into the map.
///
/// # Arguments
/// `map_ptr` - Pointer to the sample map created by `ksynth_sample_map_new`.
/// `key` - The MIDI note value (`u8`) to associate with this sample.
/// `sample_rate` - The sample rate of the sample in Hz.
/// `sample_data_ptr` - Pointer to the raw sample data (i16). Interleaved for stereo.
/// `channel` - Number of channels (1 for mono, 2 for stereo).
/// `num_samples` - Total number of i16 values.
/// `sample_loop` - Pointer to a `KSynthSampleLoop` or `null`.
///
/// # Returns
/// `true` (1) on success, `false` (0) on failure (invalid arguments, map pointer, etc.).
///
/// # Safety
/// `map_ptr` must be a valid pointer returned by `ksynth_sample_map_new`.
/// `sample_data_ptr` must point to `num_samples` valid i16 values.
/// `sample_loop`, if not null, must point to a valid `KSynthSampleLoop`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_sample_map_add_sample(
    map_ptr: *mut KSynthSampleMapPtr,
    key: u8,
    sample_rate: u32,
    sample_data_ptr: *const i16,
    channel: u8,
    num_samples: usize,
    sample_loop: *const KSynthSampleLoop,
) -> bool {
    if map_ptr.is_null() || sample_data_ptr.is_null() || (channel != 1 && channel != 2) {
        return false;
    }

    if channel == 2 && num_samples % 2 != 0 {
        return false;
    }

    let sample_map = unsafe { &mut *(map_ptr as *mut SampleMap) };

    let input_slice = unsafe { std::slice::from_raw_parts(sample_data_ptr, num_samples) };

    let sample_data = if channel == 2 {
        let stereo_data = input_slice
            .chunks_exact(2)
            .map(|chunk| (chunk[0], chunk[1]))
            .collect::<Vec<(i16, i16)>>();
        SampleData::Stereo(stereo_data)
    } else {
        SampleData::Mono(input_slice.to_vec())
    };

    let ksynth_loop = if sample_loop.is_null() {
        None
    } else {
        let loop_info = unsafe { &*sample_loop };
        Some(SampleLoop::new(loop_info.start, loop_info.end))
    };

    let sample = Sample::new(sample_rate, sample_data, ksynth_loop);

    sample_map.samples.insert(key, Arc::new(sample));
    true
}

/// Frees the memory associated with a sample map.
///
/// # Arguments
/// `map_ptr` - Pointer to the sample map to free.
///
/// # Safety
/// `map_ptr` must be a valid pointer returned by `ksynth_sample_map_new`.
/// After calling this function, the `map_ptr` becomes invalid and must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_sample_map_free(ptr: *mut KSynthSampleMapPtr) {
    if !ptr.is_null() {
        let _ = unsafe { Box::from_raw(ptr as *mut SampleMap) };
    }
}

/// Adds or replaces a sample in the specified drum kit.
///
/// This function copies the provided sample data into the kit.
/// For drum samples, it is generally recommended to pass `null` for `sample_loop`
/// as drum sounds are typically one-shot and do not loop.
///
/// # Arguments
/// `kit_ptr` - Pointer to the drum kit created by `ksynth_drum_kit_new`.
/// `key` - The MIDI note value (`u8`) to associate with this sample.
/// `sample_rate` - The sample rate of the sample in Hz.
/// `sample_data_ptr` - Pointer to the raw sample data (i16). Interleaved for stereo.
/// `channel` - Number of channels (1 for mono, 2 for stereo).
/// `num_samples` - Total number of i16 values.
/// `sample_loop` - Pointer to a `KSynthSampleLoop` or `null`.
///
/// # Returns
/// `true` (1) on success, `false` (0) on failure (invalid arguments, kit pointer, etc.).
///
/// # Safety
/// `kit_ptr` must be a valid pointer returned by `ksynth_drum_kit_new`.
/// `sample_data_ptr` must point to `num_samples` valid i16 values.
/// `sample_loop`, if not null, must point to a valid `KSynthSampleLoop`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_drum_kit_add_sample(
    kit_ptr: *mut KSynthDrumKitPtr,
    key: u8,
    sample_rate: u32,
    sample_data_ptr: *const i16,
    channel: u8,
    num_samples: usize,
    sample_loop: *const KSynthSampleLoop,
) -> bool {
    if kit_ptr.is_null() || sample_data_ptr.is_null() || (channel != 1 && channel != 2) {
        return false;
    }

    if channel == 2 && num_samples % 2 != 0 {
        return false;
    }

    let drum_kit = unsafe { &mut *(kit_ptr as *mut DrumKit) };

    let input_slice = unsafe { std::slice::from_raw_parts(sample_data_ptr, num_samples) };

    let sample_data = if channel == 2 {
        let stereo_data = input_slice
            .chunks_exact(2)
            .map(|chunk| (chunk[0], chunk[1]))
            .collect::<Vec<(i16, i16)>>();
        SampleData::Stereo(stereo_data)
    } else {
        SampleData::Mono(input_slice.to_vec())
    };

    let ksynth_loop = if sample_loop.is_null() {
        None
    } else {
        let loop_info = unsafe { &*sample_loop };
        Some(SampleLoop::new(loop_info.start, loop_info.end))
    };

    let sample = Sample::new(sample_rate, sample_data, ksynth_loop);

    drum_kit.add_sample(key, sample);
    true
}

/// Frees the memory associated with a drum kit.
///
/// # Arguments
/// `kit_ptr` - Pointer to the drum kit to free.
///
/// # Safety
/// `kit_ptr` must be a valid pointer returned by `ksynth_drum_kit_new`.
/// After calling this function, the `kit_ptr` becomes invalid and must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_drum_kit_free(ptr: *mut KSynthDrumKitPtr) {
    if !ptr.is_null() {
        let _ = unsafe { Box::from_raw(ptr as *mut DrumKit) };
    }
}

/// Creates a new `KSynth` instance using a pre-built shared sample map.
///
/// # Arguments
/// `sample_rate` - The sample rate of the synthesizer in Hz.
/// `num_channel` - The number of channels (1 for mono, 2 for stereo).
/// `max_polyphony` - The maximum number of voices.
/// `fade_out_sample` - The number of samples over which to apply the fade-out.
///                     Set to 0 to disable fade-out (no fading).
/// `sample_map_ptr` - A pointer to the shared sample map created by `ksynth_sample_map_new`.
///
/// # Returns
/// A pointer to a new `KSynth` instance, or `null` on failure.
/// The returned pointer must be freed using `ksynth_free`.
///
/// # Ownership & Memory Management
/// This function **shares** the provided sample map using `Arc::clone()`.
/// It does **not** take ownership of the `sample_map_ptr` itself.
/// The caller is still responsible for calling `ksynth_sample_map_free` on the
/// `sample_map_ptr` when it's no longer needed by the C application,
/// even after creating KSynth instances with it.
///
/// # Safety
/// `sample_map_ptr` must be a valid pointer returned by `ksynth_sample_map_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_new(
    sample_rate: u32,
    num_channel: u8,
    max_polyphony: u32,
    fade_out_sample: u64,
    sample_map_ptr: *const KSynthSampleMapPtr,
    drum_kit_ptr: *const KSynthDrumKitPtr,
) -> *mut KSynthPtr {
    if sample_map_ptr.is_null() {
        return ptr::null_mut();
    }

    let sample_map = unsafe { &*(sample_map_ptr as *const SampleMap) };

    let samples_clone = sample_map.samples.clone();

    // Convert HashMap<u8, Arc<Sample>> to HashMap<u8, Sample>
    let mut converted_samples = HashMap::new();
    for (&key, sample) in samples_clone.iter() {
        converted_samples.insert(key, (**sample).clone());
    }

    let arc_map = Arc::new(RwLock::new(converted_samples));

    let channel = match Channel::try_from(num_channel) {
        Ok(ch) => ch,
        Err(_) => return ptr::null_mut(),
    };

    let drum_kit = if drum_kit_ptr.is_null() {
        None
    } else {
        let dk = unsafe { &*(drum_kit_ptr as *const DrumKit) };
        Some(dk.clone())
    };

    let synth = Box::new(KSynth::new(
        sample_rate,
        channel,
        max_polyphony,
        fade_out_sample,
        arc_map,
        drum_kit,
    ));

    Box::into_raw(synth) as *mut KSynthPtr
}

/// Retrieves the current velocity curve from the KSynth instance and copies it into a C buffer.
///
/// The velocity curve is an array of 128 `f32` values, where the index represents
/// the MIDI velocity (0-127) and the value is the corresponding amplitude scaling factor.
///
/// # Arguments
/// `synth_ptr` - A pointer to the `KSynth` instance.
/// `out_velocity_curve` - A pointer to a C array of `float` (at least 128 elements)
///                          where the velocity curve data will be copied.
///
/// # Returns
/// `true` (1) if the velocity curve was successfully copied.
/// `false` (0) if `synth_ptr` or `out_velocity_curve` is null.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` instance previously returned by `ksynth_new`.
/// `out_velocity_curve` must be a valid pointer to a mutable block of memory capable of
///   holding at least `128 * std::mem::size_of::<f32>()` bytes. The caller is responsible
///   for allocating and managing this buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_get_velocity_curve(
    synth_ptr: *mut KSynthPtr,
    out_velocity_curve: *mut f32,
) -> bool {
    if synth_ptr.is_null() {
        return false;
    }
    if out_velocity_curve.is_null() {
        return false;
    }

    let synth = unsafe { &*(synth_ptr as *mut KSynth) };

    let velocity_curve_array: [f32; 128] = synth.get_velocity_curve();

    unsafe {
        let out_slice = std::slice::from_raw_parts_mut(out_velocity_curve, 128);
        out_slice.copy_from_slice(&velocity_curve_array);
    }

    true
}

/// Sets a new velocity curve for the KSynth instance.
///
/// The velocity curve is an array of 128 `f32` values, where the index represents
/// the MIDI velocity (0-127) and the value is the corresponding amplitude scaling factor.
/// Values provided in `new_velocity_curve_ptr` that are outside the range [0.0, 1.0]
/// will be clamped to this range by the underlying synthesizer.
///
/// # Arguments
/// `synth_ptr` - A pointer to the `KSynth` instance.
/// `new_velocity_curve_ptr` - A pointer to a C array of `float` (exactly 128 elements)
///                              containing the new velocity curve data.
///
/// # Returns
/// `true` (1) if the velocity curve was successfully set.
/// `false` (0) if `synth_ptr` or `new_velocity_curve_ptr` is null.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` instance previously returned by `ksynth_new`.
/// `new_velocity_curve_ptr` must be a valid pointer to a readable block of memory
///   containing `128` `f32` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_set_velocity_curve(
    synth_ptr: *mut KSynthPtr,
    new_velocity_curve_ptr: *const f32,
) -> bool {
    if synth_ptr.is_null() {
        return false;
    }
    if new_velocity_curve_ptr.is_null() {
        return false;
    }

    let synth = unsafe { &mut *(synth_ptr as *mut KSynth) };

    let mut new_curve_array = [0.0f32; 128];

    unsafe {
        let c_curve_slice = std::slice::from_raw_parts(new_velocity_curve_ptr, 128);
        new_curve_array.copy_from_slice(c_curve_slice);
    }

    synth.set_velocity_curve(new_curve_array);
    true
}

/// Resets the velocity curve of the KSynth instance to its default setting.
///
/// # Arguments
/// `synth_ptr` - A pointer to the `KSynth` instance.
///
/// # Returns
/// `true` (1) if the velocity curve was successfully reset.
/// `false` (0) if `synth_ptr` is null.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` instance previously returned by `ksynth_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_reset_velocity_curve(synth_ptr: *mut KSynthPtr) -> bool {
    if synth_ptr.is_null() {
        return false;
    }

    let synth = unsafe { &mut *(synth_ptr as *mut KSynth) };
    synth.reset_velocity_curve();
    true
}

/// Sets a new shared sample map for the synthesizer, replacing the existing one.
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
/// `sample_map_ptr` - A pointer to the new shared sample map.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth`.
/// `sample_map_ptr` must be a valid pointer returned by `ksynth_sample_map_new`.
/// This function clones the Arc, so the caller still needs to manage the lifetime of `sample_map_ptr` using `ksynth_sample_map_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_set_samples(
    synth_ptr: *mut KSynthPtr,
    sample_map_ptr: *const KSynthSampleMapPtr,
) {
    if synth_ptr.is_null() || sample_map_ptr.is_null() {
        return;
    }
    let synth = unsafe { &mut *(synth_ptr as *mut KSynth) };

    let sample_map = unsafe { &*(sample_map_ptr as *const SampleMap) };

    let samples_clone = sample_map.samples.clone();

    // Convert HashMap<u8, Arc<Sample>> to HashMap<u8, Sample>
    let mut converted_samples = HashMap::new();
    for (&key, sample) in samples_clone.iter() {
        converted_samples.insert(key, (**sample).clone());
    }

    let arc_map = Arc::new(RwLock::new(converted_samples));
    synth.set_samples(arc_map);
}

/// Sets a new shared drum kit for the synthesizer, replacing the existing one.
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
/// `drum_kit_ptr` - A pointer to the new shared drum kit.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth`.
/// `drum_kit_ptr` must be a valid pointer returned by `ksynth_drum_kit_new`.
/// This function clones the Arc, so the caller still needs to manage the lifetime of `drum_kit_ptr` using `ksynth_drum_kit_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_set_drum_kit(
    synth_ptr: *mut KSynthPtr,
    drum_kit_ptr: *const KSynthDrumKitPtr,
) {
    if synth_ptr.is_null() {
        return;
    }

    let synth = unsafe { &mut *(synth_ptr as *mut KSynth) };

    let drum_kit = if drum_kit_ptr.is_null() {
        None
    } else {
        let dk = unsafe { &*(drum_kit_ptr as *const DrumKit) };
        Some(dk.clone())
    };

    synth.set_drum_kit(drum_kit);
}

/// Queues a MIDI command for processing by the synthesizer.
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
/// `midi_cmd` - The MIDI command to be processed.
///              Encoded as: `status | (data1 << 8) | (data2 << 16)`
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` previously returned by `ksynth_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_queue_midi_cmd(synth_ptr: *mut KSynthPtr, midi_cmd: u32) {
    if !synth_ptr.is_null() {
        let synth_ksynth_ptr = synth_ptr as *mut KSynth;
        let synth = unsafe { &mut *synth_ksynth_ptr };

        synth.queue_midi_cmd(midi_cmd);
    }
}

/// Retrieves the fade-out sample count (in sample units).
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
///
/// # Returns
/// The fade-out sample count, or `-1` if the `synth_ptr` is invalid or fade-out is not set.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` previously returned by `ksynth_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_get_fade_out_sample(synth_ptr: *mut KSynthPtr) -> i64 {
    if !synth_ptr.is_null() {
        let synth_ksynth_ptr = synth_ptr as *mut KSynth;
        let synth = unsafe { &*synth_ksynth_ptr };
        synth.get_fade_out_sample() as i64
    } else {
        -1
    }
}

/// Sets the fade-out sample (in sample units).
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
/// `fade_out_sample` - The number of samples over which to apply the fade-out.
///                     Set to 0 to disable fade-out (no fading).
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` previously returned by `ksynth_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_set_fade_out_sample(
    synth_ptr: *mut KSynthPtr,
    fade_out_sample: u64,
) {
    if !synth_ptr.is_null() {
        let synth_ksynth_ptr = synth_ptr as *mut KSynth;
        let synth = unsafe { &mut *synth_ksynth_ptr };

        synth.set_fade_out_sample(fade_out_sample);
    }
}

/// Retrieves the current rendering time as a percentage of the available time per buffer.
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
///
/// # Returns
/// The rendering time percentage (e.g., 0.1 for 10%), or `-1.0` if the `synth_ptr` is invalid.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` previously returned by `ksynth_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_get_rendering_time(synth_ptr: *mut KSynthPtr) -> f32 {
    if !synth_ptr.is_null() {
        let synth_ksynth_ptr = synth_ptr as *mut KSynth;
        let synth = unsafe { &mut *synth_ksynth_ptr };

        synth.get_rendering_time()
    } else {
        -1.0
    }
}

/// Retrieves the current polyphony (number of voices being actively used).
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
///
/// # Returns
/// The current number of active voices, or `-1` if the `synth_ptr` is invalid.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` previously returned by `ksynth_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_get_polyphony(synth_ptr: *mut KSynthPtr) -> i32 {
    if !synth_ptr.is_null() {
        let synth_ksynth_ptr = synth_ptr as *mut KSynth;
        let synth = unsafe { &*synth_ksynth_ptr };
        synth.get_polyphony() as i32
    } else {
        -1
    }
}

/// Retrieves the polyphony counts for each MIDI channel.
///
/// # Arguments
/// * `synth_ptr` - A pointer to the KSynth instance.
/// * `out_polyphony_counts` - A pointer to an array of at least 16 u32 elements where the polyphony counts will be stored.
///
/// # Returns
/// `true` if successful, `false` if either pointer is invalid.
///
/// # Safety
/// * `synth_ptr` must be a valid pointer to a `KSynth` instance previously created by `ksynth_new`.
/// * `out_polyphony_counts` must point to a buffer capable of holding at least `16 * std::mem::size_of::<u32>()` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_get_polyphony_per_channel(
    synth_ptr: *mut KSynthPtr,
    out_polyphony_counts: *mut u32,
) -> bool {
    if synth_ptr.is_null() || out_polyphony_counts.is_null() {
        return false;
    }

    let synth_ksynth_ptr = unsafe { *(synth_ptr as *mut *mut KSynth) };
    if !synth_ksynth_ptr.is_null() {
        let synth = unsafe { &*synth_ksynth_ptr };
        let polyphony_array: [u32; 16] = synth.get_polyphony_per_channel();

        unsafe {
            let out_slice = std::slice::from_raw_parts_mut(out_polyphony_counts, 16);
            out_slice.copy_from_slice(&polyphony_array);
        }

        true
    } else {
        false
    }
}

/// Retrieves the maximum polyphony (maximum number of voices the synthesizer can handle).
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
///
/// # Returns
/// The maximum polyphony limit, or `-1` if the `synth_ptr` is invalid.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` previously returned by `ksynth_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_get_max_polyphony(synth_ptr: *mut KSynthPtr) -> i32 {
    if !synth_ptr.is_null() {
        let synth_ksynth_ptr = synth_ptr as *mut KSynth;
        let synth = unsafe { &*synth_ksynth_ptr };
        synth.get_max_polyphony() as i32
    } else {
        -1
    }
}

/// Sets the maximum polyphony (number of voices the synthesizer can handle).
///
/// If `max_polyphony` exceeds the system-supported maximum (as returned by `ksynth_get_max_supported_polyphony`),
/// it will be clamped to that upper limit (`MAX_POLYPHONY`).
/// Additionally, if `max_polyphony` is set to 0, it will be set as 1.
///
/// This function also stops all active voices, removes inactive voices, and resets the current polyphony to match
/// the number of active voices.
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
/// `max_polyphony` - The desired maximum number of voices.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` previously returned by `ksynth_new`.
/// If `max_polyphony` is set to 0, it will be set as 1.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_set_max_polyphony(synth_ptr: *mut KSynthPtr, max_polyphony: u32) {
    if !synth_ptr.is_null() {
        let synth_ksynth_ptr = synth_ptr as *mut KSynth;
        let synth = unsafe { &mut *synth_ksynth_ptr };

        synth.set_max_polyphony(max_polyphony);
    }
}

/// Fills the provided buffer with audio data generated by the synthesizer.
///
/// The buffer should be appropriately sized based on the synthesizer's channel count
/// (e.g., for stereo, `buffer_size` should be `num_frames * 2`).
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
/// `buffer` - A pointer to a buffer that will be filled with audio data (`f32`).
/// `buffer_size` - The size of the buffer in number of `f32` samples.
///
/// # Returns
/// `true` (1) if the buffer was successfully filled, or `false` (0) if the operation failed (e.g., invalid handle).
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` previously returned by `ksynth_new`.
/// `buffer` must be a valid pointer to a mutable block of memory of at least `buffer_size * std::mem::size_of::<f32>()` bytes.
/// `buffer_size` must accurately reflect the number of `f32` samples the buffer can hold.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_fill_buffer(
    synth_ptr: *mut KSynthPtr,
    buffer: *mut f32,
    buffer_size: usize,
) -> bool {
    if buffer_size == 0 {
        return false;
    }

    if !synth_ptr.is_null() {
        let synth_ksynth_ptr = synth_ptr as *mut KSynth;
        let synth = unsafe { &mut *synth_ksynth_ptr };
        let buffer_slice = unsafe { std::slice::from_raw_parts_mut(buffer, buffer_size) };
        synth.fill_buffer(buffer_slice)
    } else {
        false
    }
}

/// Frees the memory associated with a `KSynth` instance previously created by `ksynth_new`.
///
/// # Arguments
/// `synth_ptr` - A pointer to the KSynth instance.
///
/// # Safety
/// `synth_ptr` must be a valid pointer to a `KSynth` previously returned by `ksynth_new`.
/// After calling this function, the `synth_ptr` becomes invalid and must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ksynth_free(synth_ptr: *mut KSynthPtr) {
    if !synth_ptr.is_null() {
        let _ = unsafe { Box::from_raw(synth_ptr as *mut KSynth) };
    }
}
