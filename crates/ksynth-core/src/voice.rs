pub struct Voice {
    sample_index: f32,
    frames_since_release: Option<u64>,
    is_active: bool,
    is_releasing: bool,
    is_key_down: bool,
    channel: u8,
    note: u8,
    velocity: u8,
}

impl Voice {
    pub fn new(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            sample_index: 0.0,
            frames_since_release: None,
            is_active: true,
            is_releasing: false,
            is_key_down: true,
            channel,
            note,
            velocity,
        }
    }

    pub fn reuse(&mut self, channel: u8, note: u8, velocity: u8) {
        self.sample_index = 0.0;
        self.frames_since_release = None;
        self.is_active = true;
        self.is_releasing = false;
        self.is_key_down = true;
        self.channel = channel;
        self.note = note;
        self.velocity = velocity;
    }

    pub fn current_sample_index(&self) -> f32 {
        self.sample_index
    }

    pub fn increment_sample_index(&mut self, increment: f32) {
        self.sample_index += increment;
    }

    pub fn set_current_sample_index(&mut self, index: f32) {
        self.sample_index = index;
    }

    pub fn reset_sample_index(&mut self) {
        self.sample_index = 0.0;
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

    pub fn set_is_active(&mut self, is_active: bool) {
        self.is_active = is_active;
    }

    pub fn get_is_releasing(&self) -> bool {
        self.is_releasing
    }

    pub fn get_velocity(&self) -> u8 {
        self.velocity
    }

    pub fn get_is_key_down(&self) -> bool {
        self.is_key_down
    }

    pub fn set_is_key_down(&mut self, is_key_down: bool) {
        self.is_key_down = is_key_down;
    }

    pub fn increment_frames_since_release(&mut self) {
        if let Some(count) = self.frames_since_release.as_mut() {
            *count += 1;
        }
    }
    pub fn set_is_releasing(&mut self, is_releasing: bool) {
        if is_releasing && !self.is_releasing {
            self.is_releasing = true;
            self.frames_since_release = Some(0);
        } else if !is_releasing && self.is_releasing {
            self.is_releasing = false;
            self.frames_since_release = None;
        }
    }

    pub fn get_frames_since_release(&self) -> Option<u64> {
        self.frames_since_release
    }
}
