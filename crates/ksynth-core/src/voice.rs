pub struct Voice {
    pub sample_index: usize,
    pub velocity: u8,
    pub is_active: bool,
    pub is_releasing: bool,
    pub channel: u8,
    pub note: u8,
}

impl Voice {
    pub fn new(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            is_active: true,
            is_releasing: false,
            channel,
            sample_index: 0,
            note,
            velocity,
        }
    }

    pub fn current_sample_index(&self) -> usize {
        self.sample_index
    }

    pub fn increment_sample_index(&mut self) {
        self.sample_index += 1;
    }

    pub fn reset_sample_index(&mut self) {
        self.sample_index = 0;
    }

    pub fn get_velocity(&self) -> u8 {
        self.velocity
    }
}
