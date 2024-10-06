use byteorder::{ByteOrder, LittleEndian};
use hound::{self, Sample};

use log4rs::append::file;
use voice_activity_detector::{StreamExt as _, VoiceActivityDetector};
use tokio_stream::{self, StreamExt};
use tokio::io::{self, BufReader};
use tokio_util::{bytes::buf, io::ReaderStream};
use whisper_rs_test::streaming::streaming_url;
use tokio_util::{bytes::Bytes};

#[derive(Debug, PartialEq, Eq)]
enum SpeechStatus {
    Speech,
    NonSpeech,
}

fn convert_to_i16_vec(buf: &[u8]) -> Vec<i16> {
    let mut vec = Vec::with_capacity(buf.len() / 2); // Allocate space for i16 values
    for chunk in buf.chunks_exact(2) {
        vec.push(LittleEndian::read_i16(chunk));
    }
    vec
}


/*
The VAD predicts speech in a chunk of Linear Pulse Code Modulation (LPCM) encoded audio samples. These may be 8 or 16 bit integers or 32 bit floats.

The model is trained using chunk sizes of 256, 512, and 768 samples for an 8000 hz sample rate. It is trained using chunk sizes of 512, 768, 1024 samples for a 16,000 hz sample rate.
*/
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_sample_rate = 16000;

    let url = "https://rthkradio2-live.akamaized.net/hls/live/2040078/radio2/master.m3u8";
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

    // let mut nonspeech =
    //     hound::WavWriter::create("tmp/predict.stream.nonspeech.wav", spec)?;

    let mut buf:Vec<i16> = Vec::new();    
    let mut count = 1;
    let closure_annotated = |chunk: Vec<u8>| {
        eprintln!("Received chunk of size: {}", chunk.len());
        //assert!(chunk.len() == target_sample_rate);
        let samples = convert_to_i16_vec(&chunk);
        let probability = vad.predict(samples.clone());
        if probability > 0.5 {
            println!("Chunk is speech: {}", probability);
            buf.extend(&samples);
        } else {
            println!("Chunk is not speech: {}", probability);
            if buf.len() > 0 {
                let file_name = format!("tmp/predict.stream.speech.{}.wav", count);
                let mut speech = hound::WavWriter::create(file_name, spec).unwrap();
                for item in &buf {
                    speech.write_sample(*item).unwrap();
                }
                count = count + 1;
                speech.finalize().unwrap();
                buf.clear();
            }
        }
    };

    streaming_url(url,target_sample_rate,closure_annotated).await?;

    if buf.len() > 0 {
        let file_name = format!("tmp/predict.stream.speech.noend.wav");
        let mut speech = hound::WavWriter::create(file_name, spec).unwrap();
        for sample in &buf {
            speech.write_sample(*sample).unwrap();
        }
        speech.finalize().unwrap();
        buf.clear();
    }


    Ok(())
}
