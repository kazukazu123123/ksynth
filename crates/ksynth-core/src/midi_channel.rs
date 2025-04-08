#[derive(Copy, Clone)]
pub struct MidiChannel {
    channel: u8,
    pan: f32,
    rpn_msb: u8,
    rpn_lsb: u8,
    bend_range: u8,
    pitch_factor: f32,
}

impl Default for MidiChannel {
    fn default() -> Self {
        Self {
            channel: 0,
            pan: 0.0,
            rpn_msb: 0,
            rpn_lsb: 0,
            bend_range: 2,
            pitch_factor: 1.0,
        }
    }
}

impl MidiChannel {
    pub fn new(channel: u8) -> Self {
        MidiChannel {
            channel,
            pan: 0.0,
            rpn_msb: 0,
            rpn_lsb: 0,
            bend_range: 2,
            pitch_factor: 1.0,
        }
    }

    pub fn get_channel(&self) -> u8 {
        self.channel
    }

    pub fn get_pan(&self) -> f32 {
        self.pan
    }

    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan;
    }

    pub fn set_rpn_msb(&mut self, value: u8) {
        self.rpn_msb = value;
    }

    pub fn get_rpn_msb(&self) -> u8 {
        self.rpn_msb
    }

    pub fn set_rpn_lsb(&mut self, value: u8) {
        self.rpn_lsb = value;
    }

    pub fn get_rpn_lsb(&self) -> u8 {
        self.rpn_lsb
    }

    pub fn set_bend_range(&mut self, semitones: u8) {
        self.bend_range = semitones;
        println!("Pitch Bend Range set to {} semitones", semitones);
    }

    pub fn get_bend_range(&self) -> u8 {
        self.bend_range
    }

    pub fn set_pitch_factor(&mut self, pitch_factor: f32) {
        self.pitch_factor = pitch_factor;
    }

    pub fn get_pitch_factor(&self) -> f32 {
        self.pitch_factor
    }
}
