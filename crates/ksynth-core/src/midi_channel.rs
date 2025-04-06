#[derive(Copy, Clone)]
pub struct MidiChannel {
    channel: u8,
    pan: f32,
}

impl Default for MidiChannel {
    fn default() -> Self {
        Self {
            channel: 0,
            pan: 0.0,
        }
    }
}

impl MidiChannel {
    pub fn new(channel: u8) -> Self {
        MidiChannel { channel, pan: 0.0 }
    }

    pub fn get_channel(&self) -> u8 {
        self.channel
    }

    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan;
    }

    pub fn get_pan(&self) -> f32 {
        self.pan
    }
}
