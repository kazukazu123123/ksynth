#[derive(Clone, Debug)]
pub struct Sample {
    sample_rate: u32,
    sample_length: usize,
    sample_data: SampleData,
    sample_loop: Option<SampleLoop>,
}

#[derive(Clone, Debug)]
pub enum SampleData {
    Mono(Vec<i16>),
    Stereo(Vec<(i16, i16)>),
}

#[derive(Clone, Debug)]
pub struct SampleLoop {
    start: usize,
    end: usize,
}

impl SampleLoop {
    pub fn new(start: usize, end: usize) -> Self {
        assert!(start < end, "start must be less than end");
        Self { start, end }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }
}

impl Sample {
    pub fn new(sample_rate: u32, sample_data: SampleData, sample_loop: Option<SampleLoop>) -> Self {
        let sample_length = match &sample_data {
            SampleData::Mono(data) => data.len(),
            SampleData::Stereo(data) => data.len(),
        };

        Self {
            sample_rate,
            sample_length,
            sample_data,
            sample_loop,
        }
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn get_sample_data(&self) -> &SampleData {
        &self.sample_data
    }

    pub fn get_sample_loop(&self) -> Option<&SampleLoop> {
        self.sample_loop.as_ref()
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
