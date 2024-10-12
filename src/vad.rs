use std::path::PathBuf;

use ort::{GraphOptimizationLevel, Session};

use crate::sample::Sample;

/// A voice activity detector session.
#[derive(Debug)]
pub struct VoiceActivityDetector {
    chunk_size: usize,
    sample_rate: i64,
    session: ort::Session,
    h: ndarray::Array3<f32>,
    c: ndarray::Array3<f32>,
}

/// The silero ONNX model as bytes.
//const MODEL: &[u8] = include_bytes!("silero_vad.onnx");

impl VoiceActivityDetector {
    /// Create a new [VoiceActivityDetectorBuilder].
    pub fn build(sample_rate: i64,chunk_size: usize,model_file: &PathBuf) -> Self {
        let session =
            Session::builder()
                .unwrap()
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .unwrap()
                .with_intra_threads(1)
                .unwrap()
                .with_inter_threads(1)
                .unwrap()
                //.commit_from_memory(MODEL)
                .commit_from_file(model_file)
                .unwrap();

        VoiceActivityDetector {
            session,
            chunk_size: chunk_size,
            sample_rate: sample_rate,
            h: ndarray::Array3::<f32>::zeros((2, 1, 64)),
            c: ndarray::Array3::<f32>::zeros((2, 1, 64)),
        }
    }

    /// Gets the chunks size
    //pub(crate) fn chunk_size(&self) -> usize {
    //    self.chunk_size
    //}

    /// Resets the state of the voice activity detector session.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.h.fill(0f32);
        self.c.fill(0f32);
    }

    /// Predicts the existence of speech in a single iterable of audio.
    ///
    /// The samples iterator will be padded if it is too short, or truncated if it is
    /// too long.
    pub fn predict<S, I>(&mut self, samples: I) -> f32
    where
        S: Sample,
        I: IntoIterator<Item = S>,
    {
        let mut input = ndarray::Array2::<f32>::zeros((1, self.chunk_size));
        for (i, sample) in samples.into_iter().take(self.chunk_size).enumerate() {
            input[[0, i]] = sample.to_f32();
        }

        let sample_rate = ndarray::arr1::<i64>(&[self.sample_rate]);

        let inputs = ort::inputs![
            "input" => input.view(),
            "sr" => sample_rate.view(),
            "h" => self.h.view(),
            "c" => self.c.view(),
        ]
            .unwrap();

        let outputs = self.session.run(inputs).unwrap();

        // Update h and c recursively.
        let hn = outputs
            .get("hn")
            .unwrap()
            .try_extract_tensor::<f32>()
            .unwrap();
        let cn = outputs
            .get("cn")
            .unwrap()
            .try_extract_tensor::<f32>()
            .unwrap();

        self.h.assign(&hn.view());
        self.c.assign(&cn.view());

        // Get the probability of speech.
        let output = outputs
            .get("output")
            .unwrap()
            .try_extract_tensor::<f32>()
            .unwrap();
        let probability = output.view()[[0, 0]];

        probability
    }
}
