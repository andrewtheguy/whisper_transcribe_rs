
use hound::{self, Sample};
use serde_json::json;
use std::{fs::File, process, env};
use std::io::Write;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use std::process::{Command, Stdio};
use std::io::{Read, Cursor};
use std::convert::TryInto;
use whisper_rs_test::convert_file_to_wave;
use voice_activity_detector::{LabeledAudio, IteratorExt, VoiceActivityDetector};

#[derive(Debug, PartialEq, Eq)]
enum SpeechStatus {
    Speech,
    NonSpeech,
}



/*
The VAD predicts speech in a chunk of Linear Pulse Code Modulation (LPCM) encoded audio samples. These may be 8 or 16 bit integers or 32 bit floats.

The model is trained using chunk sizes of 256, 512, and 768 samples for an 8000 hz sample rate. It is trained using chunk sizes of 512, 768, 1024 samples for a 16,000 hz sample rate.
*/
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_sample_rate = 16000;

    // Collect command-line arguments, skipping the first one (program's name)
    let args: Vec<String> = env::args().collect();

    // Check if the first argument (after the program name) is provided
    if args.len() < 2 {
        eprintln!("Usage: {} <argument>", args[0]);
        process::exit(1);
    }

    // Get the first argument (the second element in the args vector)
    let input_file = &args[1];
    //println!("First argument: {}", first_argument);


    let samples = convert_file_to_wave(input_file,target_sample_rate)?;


    //let samples = [0i16; 51200];
    let vad = VoiceActivityDetector::builder()
        .sample_rate(target_sample_rate)
        .chunk_size(512usize)
        .build()?;

    let chunk_duration = 512.0 / target_sample_rate as f32; // Duration of each chunk in seconds
    let mut chunk_index = 0;

    // This will label any audio chunks with a probability greater than 50% as speech,
    // and label the 3 additional chunks before and after these chunks as speech.
    let labels = samples.into_iter().label(vad, 0.5, 3);

    let mut ts_array = Vec::new();

    let mut cur_speech_start:Option<f32> = None;

    let mut cur_status = SpeechStatus::NonSpeech;

    // Process each labeled chunk and print whether speech or non-speech is detected with timestamp
    for label in labels {
        let timestamp = chunk_index as f32 * chunk_duration; // Calculate timestamp for the current chunk
        match label {
            LabeledAudio::Speech(_) => {
                if cur_status == SpeechStatus::NonSpeech {
                    cur_speech_start = Some(timestamp);
                    cur_status = SpeechStatus::Speech;
                }
                println!("Speech detected at {:.2} seconds", timestamp)
            },
            LabeledAudio::NonSpeech(_) => {
                if cur_status == SpeechStatus::Speech {
                    let cur_speech_start = cur_speech_start.unwrap();
                    let cur_speech_end = timestamp;
                    let ts = (cur_speech_start,cur_speech_end);
                    ts_array.push(ts);
                    cur_status = SpeechStatus::NonSpeech;
                }
                println!("Non-speech detected at {:.2} seconds", timestamp)
            },
        }
        chunk_index += 1; // Increment chunk index
    }
    if let Some(cur_speech_start) = cur_speech_start {
        if cur_status == SpeechStatus::Speech {
            let cur_speech_end = chunk_index as f32 * chunk_duration;
            let ts = (cur_speech_start,cur_speech_end);
            ts_array.push(ts);
        }
    }
    eprint!("ts_array: {:?}",ts_array);
    Ok(())
}
