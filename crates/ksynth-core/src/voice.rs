pub struct Voice {
    sample_index: usize,
    release_start_index: Option<usize>,
    is_active: bool,
    is_releasing: bool,
    channel: u8,
    note: u8,
    velocity: u8,
}

impl Voice {
    pub fn new(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            sample_index: 0,
            release_start_index: None,
            is_active: true,
            is_releasing: false,
            channel,
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

    pub fn set_current_sample_index(&mut self, index: usize) {
        self.sample_index = index;
    }

    pub fn reset_sample_index(&mut self) {
        self.sample_index = 0;
    }

    pub fn get_channel(&self) -> u8 {
        self.channel
    }

    pub fn get_note(&self) -> u8 {
        self.note
    }

    pub fn get_is_active(&self) -> bool {
        self.is_active
    }

    pub fn get_is_releasing(&self) -> bool {
        self.is_releasing
    }

    pub fn get_velocity(&self) -> u8 {
        self.velocity
    }

    pub fn set_is_active(&mut self, is_active: bool) {
        self.is_active = is_active;
    }

    pub fn set_is_releasing(&mut self, is_releasing: bool) {
        self.is_releasing = is_releasing;
        if is_releasing {
            self.release_start_index = Some(self.sample_index);
        } else {
            self.release_start_index = None;
        }
    }

    pub fn get_release_start_index(&self) -> Option<usize> {
        self.release_start_index
    }
}
