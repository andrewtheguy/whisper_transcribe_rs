use byteorder::{ByteOrder, LittleEndian};
use chrono::{DateTime, TimeZone, Utc};
use crossbeam::channel::{Receiver, Sender};
use log::{debug, error, info, trace, warn};
use serde_json::json;
use std::{io::{BufReader, Read}, process::{Command, Stdio}, thread::sleep};
use serde::{Deserialize, Serialize};
use std::str;
use read_chunks::ReadExt;
use std::time::{SystemTime, UNIX_EPOCH};


use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, Stream};
use cpal::{Sample, StreamConfig};

pub struct Segment {
    pub timestamp: i64,
    pub samples: Vec<i16>,
}

fn convert_to_i16_vec(buf: &[u8]) -> Vec<i16> {
    let mut vec = Vec::with_capacity(buf.len() / 2); // Allocate space for i16 values
    for chunk in buf.chunks_exact(2) {
        vec.push(LittleEndian::read_i16(chunk));
    }
    vec
}

#[derive(Debug, Serialize, Deserialize)]
struct FFProbeFormat {
    duration: Option<String>, // duration is stored as a string in ffprobe JSON output
}

#[derive(Debug, Serialize, Deserialize)]
struct FFProbeOutput {
    format: FFProbeFormat,
}

fn setup_audio_play(rxaudio: Receiver<f32>) -> Result<Stream, Box<dyn std::error::Error> > {


    // Initialize the CPAL host
    let host = cpal::default_host();

    // Get the default output device
    let output_device = host.default_output_device().ok_or("No output device available")?;
    
    // Get the default output format
    let output_format = output_device.default_output_config()?;
    println!("Default output format: {:?}", output_format);
    
    // Define the desired output configuration
    let desired_config = StreamConfig {
        channels: 1, // Mono
        sample_rate: cpal::SampleRate(16_000), // 16 kHz
        // set buffer size according to the output configuration
        buffer_size: cpal::BufferSize::Default,
    };

    // Create a stream to play audio
    let mut stream = output_device.build_output_stream(
        &desired_config,
        move |data: &mut [f32], _| {
            for sample in data {
                *sample = match rxaudio.recv() {
                    Ok(sample) => sample,
                    Err(_) => {
                        eprintln!("Error reading from channel");
                        return;
                    },
                };
            }
        },
        |err| {
            eprintln!("Error occurred on stream: {:?}", err);
        },
        None // None=blocking, Some(Duration)=timeout
    )?;

    Ok(stream)
}


fn streaming_inner_loop(input_url: &str, target_sample_rate: i64, sample_size: usize, tx: &Sender<Option<Segment>>,is_live_stream: bool, txaudio: Option<&Sender<f32>>) -> Result<(), Box<dyn std::error::Error>>
{
    // Path to the input file
    //let input_file = "input.mp3"; // Replace with your file path

    // Run ffmpeg to get raw PCM (s16le) data at 16kHz
    let mut ffmpeg_process = Command::new("ffmpeg")
        .args(&[
            //-drop_pkts_on_overflow 1 
            "-i", input_url,      // Input url
            "-attempt_recovery", "1",
            "-hide_banner",
            "-loglevel", "error",
            "-recovery_wait_time", "1",
            "-f", "s16le",         // Output format: raw PCM, signed 16-bit little-endian
            "-acodec", "pcm_s16le",// Audio codec: PCM 16-bit signed little-endian
            "-ac", "1",            // Number of audio channels (1 = mono)
            "-ar", &format!("{}",target_sample_rate),        // Sample rate: 16 kHz
            "-"                    // Output to stdout
        ])
        .stdout(Stdio::piped())
        //.stderr(Stdio::null()) // Optional: Ignore stderr output
        .spawn()?;

    let current_timestamp = Utc::now();
    debug!("{}",json!({"start_timestamp": current_timestamp.to_rfc3339()}));


    // Get a handle to the stdout of the child process
    let stdout = ffmpeg_process.stdout.take().expect("child process did not have a handle to stdout");

    // Create a buffered reader for the stdout of the child process
    let mut reader = BufReader::new(stdout);


    // 16 kHz * 2 bytes per sample * 1 channels
    //let one_second: usize = (target_sample_rate * 2 * 1).try_into().unwrap(); 
    // Buffer for reading 16,000 * 2 bytes

    //let mut buffer = vec![0u8; sample_size*2];

    let start_ts_millis = Utc::now().timestamp_millis();

    let datetime = Utc.timestamp_millis_opt(start_ts_millis).unwrap();

    eprintln!("Start ts: {}",datetime);

    let mut num_samples = 0;


    while let Some(chunk_result) = reader.read_chunks(sample_size*2).next_chunk() {
        // don't allow live stream (url with unlimited duration) to be too backed up
        if is_live_stream && tx.is_full() {
            panic!("Channel is full for livestream, transcribe thread not being able to catch up, aborting");
        }

        trace!("{}",json!({"channel_size": tx.len()}).to_string());
        
        let chunk = chunk_result?;

        let sample = convert_to_i16_vec(&chunk);

        num_samples = num_samples+sample.len();

        let ts_new = num_samples as f64/target_sample_rate as f64 * 1000.0;

        let cur_ts = start_ts_millis+ts_new.round() as i64;

        //let datetime = Utc.timestamp_millis_opt(cur_ts).unwrap();

        //eprintln!("cur_ts: {}",datetime);

        // operation thread
        tx.send(Some(Segment{
            timestamp: cur_ts,
            samples: sample.clone(),
        }))?;

        // play audio thread
        if let Some(txaudio) = &txaudio {
            for s in sample {
                let sample2: f32 = f32::from_sample(s);
                txaudio.send(sample2)?;
            }
        }
    }

    // Wait for the child process to finish
    let status = ffmpeg_process.wait()?;
    error!("ffmpeg exited with status: {}", status);
    if !status.success() {
        return Err(format!("ffmpeg failed with a non-zero exit code {}", status.code().unwrap_or(-1)).into());
    }

    Ok(())
}


pub fn streaming_url(input_url: &str, target_sample_rate: i64, sample_size: usize,tx: &Sender<Option<Segment>>) -> Result<(), Box<dyn std::error::Error>>
{

    // Run ffmpeg to get raw PCM (s16le) data at 16kHz
    let output = Command::new("ffprobe")
        .args(&[
            //-drop_pkts_on_overflow 1 
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "json",
            input_url,      // Input url
        ])
        .output()?;


    // Convert the output to a string
    let stdout = str::from_utf8(&output.stdout)?;

    // Parse the JSON output
    let ffprobe_output: FFProbeOutput = serde_json::from_str(stdout)?;

    let status = output.status;
    if !status.success() {
        error!("ffprobe failed with a non-zero exit code: {}", status);
        return Err(format!("ffmpeg failed with a non-zero exit code {}", status.code().unwrap_or(-1)).into());
    }


    // Check if duration exists and print it
    if let Some(duration) = ffprobe_output.format.duration {
        debug!("Duration: {} seconds", duration);
        streaming_inner_loop(input_url, target_sample_rate, sample_size, &tx, false,None)?;

        // Send none to signal the end of the stream
        tx.send(None)?;
    } else {


        let (txaudio, rxaudio) = crossbeam::channel::bounded::<f32>(1024);

        let stream = setup_audio_play(rxaudio)?;

        // Start the stream
        stream.play()?;

        info!("No duration found, assuming stream is infinite and will restart on stream stop");
        loop {
            streaming_inner_loop(input_url, target_sample_rate, sample_size, &tx,true,Some(&txaudio))?;
            warn!("stream_stopped, restarting");
            sleep(std::time::Duration::from_millis(500));
        }
    }

    Ok(())
}
