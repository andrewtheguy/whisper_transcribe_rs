use std::pin::{pin, Pin};

use byteorder::{ByteOrder, LittleEndian};
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


fn convert_to_i16_vec(buf: &[u8]) -> Vec<i16> {
    let mut vec = Vec::with_capacity(buf.len() / 2); // Allocate space for i16 values
    for chunk in buf.chunks_exact(2) {
        vec.push(LittleEndian::read_i16(chunk));
    }
    vec
}

pub async fn process_buffer_with_vad<F>(url: &str,target_sample_rate: i32, mut f: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(&Vec<i16>),
{
    //let target_sample_rate: i32 = 16000;

    let mut vad = VoiceActivityDetector::builder()
        .sample_rate(target_sample_rate)
        .chunk_size(1024usize)
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
    let closure_annotated = |chunk: Vec<u8>| {

        eprintln!("Received chunk of size: {}", chunk.len());
        //assert!(chunk.len() as i32 == target_sample_rate * 2); //make sure it is one second
        //cur_seconds += 1;
        let samples = convert_to_i16_vec(&chunk);
        eprintln!("sample size: {}", samples.len());
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

    streaming_url(url,target_sample_rate,Box::new(closure_annotated)).await?;

    if buf.len() > 0 {
        f(&buf);
        buf.clear();
        num += 1;
    }


    Ok(())
}
