use hound::{self, Sample};

use tokio_stream::{self, StreamExt};
use tokio::io::{self, BufReader};
use tokio_util::{bytes::buf, io::ReaderStream};
use crate::{silero, streaming::streaming_url, utils};
use tokio_util::{bytes::Bytes};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use rand::Rng;

enum State {
    NoSpeech,
    HasSpeech,
}

impl State {
    fn convert(has_speech: bool) -> State {
        match has_speech {
            true => State::HasSpeech,
            _ => State::NoSpeech,
        }
    }
}

pub async fn process_buffer_with_vad<F>(url: &str,target_sample_rate: i64, sample_size: usize, mut f: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(&Vec<i16>),
{
    //let target_sample_rate: i32 = 16000;

    let model_path = std::env::var("SILERO_MODEL_PATH")
        .unwrap_or_else(|_| String::from("./models/silero_vad.onnx"));

    let sample_rate = match target_sample_rate {
        8000 => utils::SampleRate::EightkHz,
        16000 => utils::SampleRate::SixteenkHz,
        _ => panic!("Unsupported sample rate. Expect 8 kHz or 16 kHz."),
    };

    let mut silero = silero::Silero::new(sample_rate, model_path).unwrap();

    let mut buf:Vec<i16> = Vec::new();
    let mut num = 1;

    let min_speech_duration_seconds = 3.0;

    let mut has_speech = false;

    let mut prev_sample:Option<Vec<i16>> = None;

    //let mut has_speech_time = 0.0;

    let mut prev_state = State::NoSpeech;

    //let whisper_wrapper_ref = RefCell::new(whisper_wrapper);
    //let whisper_wrapper_ref2 = &whisper_wrapper;
    let closure_annotated = |samples: Vec<i16>| {
        eprintln!("Received sample size: {}", samples.len());
        //assert!(samples.len() as i32 == target_sample_rate); //make sure it is one second
        //let sample2 = samples.clone();
        //silero.reset();
        //let mut rng = rand::thread_rng();
        //let probability: f64 = rng.gen();
        let probability = silero.calc_level(&samples).unwrap();
        //let len_after_samples: i32 = (buf.len() + samples.len()).try_into().unwrap();
        eprintln!("buf.len() {}", buf.len());
        let seconds = buf.len() as f32 / target_sample_rate as f32;
        //eprintln!("len_after_samples / target_sample_rate {}",seconds);

        if probability > 0.5 {
            eprintln!("Chunk is speech: {}", probability);
            has_speech = true;
        } else {
            has_speech = false;
        }

        match prev_state {
            State::NoSpeech => {
                if has_speech {
                    eprintln!("Transitioning from no speech to speech");
                    // add previous sample if it exists
                    if let Some(prev_sample2) = &prev_sample {
                        buf.extend(prev_sample2);
                    }
                    // start to extend the buffer
                    buf.extend(&samples);
                } else {
                    eprintln!("Still No Speech");
                }
            },
            State::HasSpeech => {
                if seconds < min_speech_duration_seconds {
                    eprintln!("override to Continue to has speech because seconds < min_seconds {}", seconds);
                    has_speech = true;
                }
                if has_speech {
                    eprintln!("Continue to has speech");
                    // continue to extend the buffer
                    buf.extend(&samples);
                } else {
                    eprintln!("Transitioning from speech to no speech");
                    buf.extend(&samples);
                    //save the buffer if not empty
                    f(&buf);
                    buf.clear();
                    num += 1;
                }
            }
        }

        prev_state = State::convert(has_speech);
        
        prev_sample = Some(samples);
    };

    streaming_url(url,target_sample_rate,sample_size,closure_annotated).await?;

    if buf.len() > 0 {
        f(&buf);
        buf.clear();
        num += 1;
    }


    Ok(())
}
