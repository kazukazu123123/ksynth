use super::midi_channel::MidiChannel;

pub fn handle_control_change(midi_channel: &mut MidiChannel, data1: u8, data2: u8) {
    match data1 {
        // Pan
        0x0A => {
            let pan = (data2 as f32 / 127.0) * 2.0 - 1.0;
            midi_channel.set_pan(pan);
        }
        // Damper pedal
        0x40 => {
            let is_sustain = data2 > 63;
            midi_channel.set_sustain(is_sustain);
        }
        // Volume
        0x07 => {
            midi_channel.set_volume(data2);
        }
        // RPN MSB: CC101 = 0
        0x65 => {
            // CC101 (RPN MSB)
            if data2 == 0 {
                midi_channel.set_rpn_msb(0);
            }
        }
        // RPN LSB: CC100 = 0
        0x64 => {
            // CC100 (RPN LSB)
            if data2 == 0 {
                midi_channel.set_rpn_lsb(0);
            }
        }
        // Data Entry MSB: CC6 (Pitch Bend Sensitivity)
        0x06 => {
            // CC6 (Data Entry)
            if midi_channel.get_rpn_msb() == 0 && midi_channel.get_rpn_lsb() == 0 {
                // Set pitch bend range in semitones
                let bend_range = data2 as u8;
                midi_channel.set_bend_range_semitone(bend_range);
            }
        }
        _ => {}
    }
}
