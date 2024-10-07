use std::pin::{pin, Pin};

use hound::{self, Sample};

use log4rs::append::file;
use serde_json::json;
use tokio_stream::{self, StreamExt};
use tokio::io::{self, BufReader};
use tokio_util::{bytes::buf, io::ReaderStream};
use crate::{silero, streaming::streaming_url, utils};
use tokio_util::{bytes::Bytes};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use rand::Rng;

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
    // should add speech and silence threshold as well
    let min_seconds = 10.0;
    let max_seconds = 30.0;
    let mut last_uncommitted_no_speech: Option<Vec<i16>> = None;

    let mut prev_has_speech = false;
    let mut has_speech = false;

    //let whisper_wrapper_ref = RefCell::new(whisper_wrapper);
    //let whisper_wrapper_ref2 = &whisper_wrapper;
    let closure_annotated = |samples: Vec<i16>| {
        eprintln!("Received sample size: {}", samples.len());
        //assert!(samples.len() as i32 == target_sample_rate); //make sure it is one second
        //let sample2 = samples.clone();
        //silero.reset();
        let mut rng = rand::thread_rng();
        let probability: f64 = rng.gen();
        //let probability = silero.calc_level(&sample2).unwrap();
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

        if(has_speech) {
            eprintln!("Chunk is speech: {}", probability);
            buf.extend(&samples);
            if seconds > max_seconds {
                eprintln!("too long, saving and treating has no speech:");
                buf.extend(&samples);
                //let file_name = format!("tmp/predict.stream.speech.{}.wav", num);
                f(&buf);
                buf.clear();
                num += 1;
                prev_has_speech = false;
            }
        } else if (prev_has_speech && !has_speech) {
            eprintln!("Chunk is transitioning from speech to not speech: {}", probability);
            if seconds < min_seconds {
                eprintln!("too short:");
                buf.extend(&samples);
            } else {
                eprintln!("not too short, saving:");
                buf.extend(&samples);
                //let file_name = format!("tmp/predict.stream.speech.{}.wav", num);
                f(&buf);
                buf.clear();
                num += 1;
            }
        }

        prev_has_speech = has_speech;
    };

    streaming_url(url,target_sample_rate,sample_size,Box::new(closure_annotated)).await?;

    if buf.len() > 0 {
        f(&buf);
        buf.clear();
        num += 1;
    }


    Ok(())
}
