use byteorder::{ByteOrder, LittleEndian};
use crossbeam::channel::Sender;
use serde_json::json;
use std::{io::{BufReader, Read}, process::{Command, Stdio}, thread::sleep};
use serde::{Deserialize, Serialize};
use std::str;
use read_chunks::ReadExt;

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



fn streaming_inner_loop(input_url: &str, target_sample_rate: i64, sample_size: usize, tx: &Sender<Option<Vec<i16>>>,is_live_stream: bool) -> Result<(), Box<dyn std::error::Error>>
{
    // Path to the input file
    //let input_file = "input.mp3"; // Replace with your file path

    // Run ffmpeg to get raw PCM (s16le) data at 16kHz
    let mut ffmpeg_process = Command::new("ffmpeg")
        .args(&[
            //-drop_pkts_on_overflow 1 
            "-i", input_url,      // Input url
            "-attempt_recovery", "1",
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


    // Get a handle to the stdout of the child process
    let stdout = ffmpeg_process.stdout.take().expect("child process did not have a handle to stdout");

    // Create a buffered reader for the stdout of the child process
    let mut reader = BufReader::new(stdout);


    // 16 kHz * 2 bytes per sample * 1 channels
    //let one_second: usize = (target_sample_rate * 2 * 1).try_into().unwrap(); 
    // Buffer for reading 16,000 * 2 bytes

    //let mut buffer = vec![0u8; sample_size*2];


    while let Some(chunk) = reader.read_chunks(sample_size*2).next_chunk() {
        // don't allow live stream (url with unlimited duration) to be too backed up
        if is_live_stream && tx.is_full() {
            panic!("Channel is full for livestream, transcribe thread not being able to catch up, aborting");
        }

        println!("{}",json!({"channel_size": tx.len()}).to_string());
        tx.send(convert_to_i16_vec(&chunk?))?;

    }

    // Wait for the child process to finish
    let status = ffmpeg_process.wait()?;
    eprintln!("ffmpeg exited with status: {}", status);
    if !status.success() {
        return Err(format!("ffmpeg failed with a non-zero exit code {}", status.code().unwrap_or(-1)).into());
    }

    Ok(())
}


pub fn streaming_url(input_url: &str, target_sample_rate: i64, sample_size: usize,tx: &Sender<Option<Vec<i16>>>) -> Result<(), Box<dyn std::error::Error>>
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
        eprintln!("ffprobe failed with a non-zero exit code: {}", status);
        return Err(format!("ffmpeg failed with a non-zero exit code {}", status.code().unwrap_or(-1)).into());
    }


    // Check if duration exists and print it
    if let Some(duration) = ffprobe_output.format.duration {
        eprintln!("Duration: {} seconds", duration);
        streaming_inner_loop(input_url, target_sample_rate, sample_size, &tx, false)?;

        // Send none to signal the end of the stream
        tx.send(None)?;
    } else {
        eprintln!("No duration found, assuming stream is infinite and will restart on stream stop");
        loop {
            streaming_inner_loop(input_url, target_sample_rate, sample_size, &tx,true)?;
            eprintln!("stream_stopped, restarting");
            sleep(std::time::Duration::from_millis(500));
        }
    }

    Ok(())
}
