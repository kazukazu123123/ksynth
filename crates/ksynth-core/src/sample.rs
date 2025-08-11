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

    pub fn resample(&self, target_sample_rate: u32) -> Self {
        if self.sample_rate == target_sample_rate {
            return self.clone();
        }

        let ratio = self.sample_rate as f64 / target_sample_rate as f64;

        let new_length = (self.sample_length as f64 / ratio).ceil() as usize;

        let new_sample_data = match &self.sample_data {
            SampleData::Mono(data) => {
                let mut new_data = Vec::with_capacity(new_length);

                for i in 0..new_length {
                    let src_pos = i as f64 * ratio;
                    let src_idx = src_pos.floor() as usize;
                    let frac = src_pos - src_idx as f64;

                    if src_idx + 1 < data.len() {
                        let s1 = data[src_idx] as f64;
                        let s2 = data[src_idx + 1] as f64;
                        let interpolated = s1 + (s2 - s1) * frac;
                        new_data.push(interpolated as i16);
                    } else if src_idx < data.len() {
                        new_data.push(data[src_idx]);
                    } else {
                        new_data.push(0);
                    }
                }

                SampleData::Mono(new_data)
            }
            SampleData::Stereo(data) => {
                let mut new_data = Vec::with_capacity(new_length);

                for i in 0..new_length {
                    let src_pos = i as f64 * ratio;
                    let src_idx = src_pos.floor() as usize;
                    let frac = src_pos - src_idx as f64;

                    if src_idx + 1 < data.len() {
                        let (l1, r1) = data[src_idx];
                        let (l2, r2) = data[src_idx + 1];

                        let l_interpolated = l1 as f64 + (l2 as f64 - l1 as f64) * frac;
                        let r_interpolated = r1 as f64 + (r2 as f64 - r1 as f64) * frac;

                        new_data.push((l_interpolated as i16, r_interpolated as i16));
                    } else if src_idx < data.len() {
                        new_data.push(data[src_idx]);
                    } else {
                        new_data.push((0, 0));
                    }
                }

                SampleData::Stereo(new_data)
            }
        };

        let new_sample_loop = self.sample_loop.as_ref().map(|loop_info| {
            let new_start = (loop_info.start() as f64 / ratio).round() as usize;
            let new_end = (loop_info.end() as f64 / ratio).round() as usize;
            SampleLoop::new(new_start, new_end.max(new_start + 1))
        });

        Self {
            sample_rate: target_sample_rate,
            sample_length: new_length,
            sample_data: new_sample_data,
            sample_loop: new_sample_loop,
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
