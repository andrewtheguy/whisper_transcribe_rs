use core::num;
use std::{cell::RefCell, rc::Rc};

use byteorder::{ByteOrder, LittleEndian};
use hound::{self, Sample};

use log4rs::append::file;
use serde_json::json;
use voice_activity_detector::{StreamExt as _, VoiceActivityDetector};
use tokio_stream::{self, StreamExt};
use tokio::io::{self, BufReader};
use tokio_util::{bytes::buf, io::ReaderStream};
use whisper_rs_test::{streaming::streaming_url, vad_processor::process_buffer_with_vad};
use tokio_util::{bytes::Bytes};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn sync_buf_to_file(buf: &Vec<i16>, file_name: &str) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(file_name, spec).unwrap();
    for sample in buf {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
}


/*
The VAD predicts speech in a chunk of Linear Pulse Code Modulation (LPCM) encoded audio samples. These may be 8 or 16 bit integers or 32 bit floats.

The model is trained using chunk sizes of 256, 512, and 768 samples for an 8000 hz sample rate. It is trained using chunk sizes of 512, 768, 1024 samples for a 16,000 hz sample rate.
*/
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_sample_rate: i32 = 16000;

    //let url = "https://rthkradio2-live.akamaized.net/hls/live/2040078/radio2/master.m3u8";
    let url = "https://www.am1430.net/wp-content/uploads/show/%E7%B9%BC%E7%BA%8C%E6%9C%89%E5%BF%83%E4%BA%BA/2023/2024-10-03.mp3";
    //println!("First argument: {}", first_argument);



    //let samples = [0i16; 51200];
    let mut vad = VoiceActivityDetector::builder()
        .sample_rate(target_sample_rate)
        .chunk_size(512usize)
        .build()?;


    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: target_sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

      
    /*

    let mut buf:Vec<i16> = Vec::new();    
    let mut num = 1;
    let max_seconds = 10;
    let mut last_uncommitted_no_speech: Option<Vec<i16>> = None;

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

            let file_name = format!("tmp/predict.stream.speech.{}.wav", num);
            sync_buf_to_file(&mut buf, &file_name);

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

                let file_name = format!("tmp/predict.stream.speech.{}.wav", num);
                sync_buf_to_file(&mut buf, &file_name);
      
                num += 1;
            }else{ //not committed yet
                last_uncommitted_no_speech = Some(samples);
            }
        }
    };

    streaming_url(url,target_sample_rate,Box::new(closure_annotated)).await?;

    if buf.len() > 0 {
  
        let file_name = format!("tmp/predict.stream.speech.noend.wav");
        sync_buf_to_file(&mut buf, &file_name);

        num += 1;
    }


    */




    let mut num = 1;
    let closure_annotated = |buf: &Vec<i16>| {
        let file_name = format!("tmp/predict.stream.speech.{}.wav", num);
        sync_buf_to_file(&buf, &file_name);
        num += 1;
    };

    process_buffer_with_vad(url,target_sample_rate,closure_annotated).await?;



    Ok(())
}
