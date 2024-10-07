use std::pin::{pin, Pin};

use hound::{self, Sample};

use log4rs::append::file;
use serde_json::json;
use voice_activity_detector::{StreamExt as _, VoiceActivityDetector};
use tokio_stream::{self, StreamExt};
use tokio::io::{self, BufReader};
use tokio_util::{bytes::buf, io::ReaderStream};
use crate::streaming::streaming_url;
use tokio_util::{bytes::Bytes};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

pub async fn process_buffer_with_vad<F>(url: &str,target_sample_rate: i64, sample_size: usize, mut f: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(&Vec<i16>),
{
    //let target_sample_rate: i32 = 16000;

    let mut vad = VoiceActivityDetector::builder()
        .sample_rate(target_sample_rate)
        .chunk_size(sample_size)
        .build()?;

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
        let probability = vad.predict(samples.clone());
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

        buf.extend(&samples);
        if(has_speech) {
            eprintln!("Chunk is speech: {}", probability);
            if seconds > max_seconds {
                eprintln!("too long, saving and treating has no speech:");
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
            } else {
                eprintln!("not too short, saving:");
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
