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
    .chunk_size(512usize)
    .build()?;

    let mut buf:Vec<i16> = Vec::new();
    let mut num = 1;
    let max_seconds = 10;
    let mut last_uncommitted_no_speech: Option<Vec<i16>> = None;

    //let whisper_wrapper_ref = RefCell::new(whisper_wrapper);
    //let whisper_wrapper_ref2 = &whisper_wrapper;
    let closure_annotated = |chunk: Vec<u8>| {

        eprintln!("Received chunk of size: {}", chunk.len());
        //assert!(chunk.len() as i32 == target_sample_rate * 2); //make sure it is one second
        //cur_seconds += 1;
        let samples = convert_to_i16_vec(&chunk);
        //assert!(samples.len() as i32 == target_sample_rate); //make sure it is one second
        let probability = vad.predict(samples.clone());
        let len_after_samples: i32 = (buf.len() + samples.len()).try_into().unwrap();
        if buf.len() > 0 && (len_after_samples / target_sample_rate) % max_seconds == 0 {
            eprintln!("Chunk is more than {} seconds, flushing", max_seconds);
            //add the last uncommitted no speech first
            if let Some(last_uncommitted_no_speech2) = &last_uncommitted_no_speech {
                buf.extend(last_uncommitted_no_speech2);
                last_uncommitted_no_speech = None;
            }
            buf.extend(&samples);
            f(&buf);
            buf.clear();
            num += 1;
            //cur_seconds = 0;
        } else if probability > 0.5 {
            eprintln!("Chunk is speech: {}", probability);
            //add the last uncommitted no speech first
            if let Some(last_uncommitted_no_speech2) = &last_uncommitted_no_speech {
                buf.extend(last_uncommitted_no_speech2);
                last_uncommitted_no_speech = None;
            }
            buf.extend(&samples);
        } else {
            eprintln!("Chunk is not speech: {}", probability);
            if buf.len() > 0 {
                buf.extend(&samples);
                last_uncommitted_no_speech = None;
                f(&buf);
                buf.clear();
                num += 1;
            }else{ //not committed yet
                last_uncommitted_no_speech = Some(samples);
            }
        }
    };

    streaming_url(url,target_sample_rate,Box::new(closure_annotated)).await?;

    if buf.len() > 0 {
        f(&buf);
        buf.clear();
        num += 1;
    }


    Ok(())
}
