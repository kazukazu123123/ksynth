#[derive(Clone, Debug)]
pub struct Sample {
    sample_rate: u32,
    sample_length: usize,
    sample_data: SampleData,
}

#[derive(Clone, Debug)]
pub enum SampleData {
    Mono(Vec<i16>),
    Stereo(Vec<(i16, i16)>),
}

impl Sample {
    pub fn new(sample_rate: u32, sample_data: SampleData) -> Self {
        let sample_length = match &sample_data {
            SampleData::Mono(data) => data.len(),
            SampleData::Stereo(data) => data.len(),
        };

        // Check if the length of mono and stereo are the same
        let sample_length_mono = match &sample_data {
            SampleData::Mono(data) => data.len(),
            SampleData::Stereo(data) => data.len(),
        };

        assert_eq!(sample_length, sample_length_mono);

        Self {
            sample_rate,
            sample_length,
            sample_data,
        }
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn get_sample_data(&self) -> &SampleData {
        &self.sample_data
    }

    pub fn downmix(self) -> Vec<i16> {
        match self.sample_data {
            SampleData::Mono(data) => data,
            SampleData::Stereo(data) => data
                .into_iter()
                .map(|(l, r)| ((l + r) / 2) as i16)
                .collect(),
        }
    }

    pub fn sample_length(&self) -> usize {
        self.sample_length
    }

    pub fn is_empty(&self) -> bool {
        match &self.sample_data {
            SampleData::Mono(data) => data.is_empty(),
            SampleData::Stereo(data) => data.is_empty(),
        }
    }
}
