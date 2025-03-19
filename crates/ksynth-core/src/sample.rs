#[derive(Clone, Debug)]
pub struct Sample {
    pub sample_data: Vec<i16>,
}

impl Sample {
    pub fn new(data: Vec<i16>) -> Self {
        Self { sample_data: data }
    }

    pub fn len(&self) -> usize {
        self.sample_data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sample_data.is_empty()
    }
}
