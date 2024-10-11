use byteorder::{ByteOrder, LittleEndian};
use tokio::{process::Command};
use std::{process::Stdio, time::Instant};
use tokio::io::{AsyncReadExt, BufReader};
use serde::{Deserialize, Serialize};
use std::str;

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



async fn streaming_inner_loop<F>(input_url: &str, target_sample_rate: i64, sample_size: usize,mut f: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(Vec<i16>),
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
    // Buffer for reading 16,000 bytes
    
    let mut buffer = vec![0u8; sample_size*2]; 
    let mut total_bytes_in_buffer = 0;

    loop {
        // Read as much as possible to fill the remaining space in the buffer
        let bytes_read = reader.read(&mut buffer[total_bytes_in_buffer..]).await?;
        
        // If no more bytes are read, we're done
        if bytes_read == 0 {
            // If there's any remaining data in the buffer, process it as the last chunk
            if total_bytes_in_buffer > 0 {
                let slice = &buffer[..total_bytes_in_buffer];
                f(convert_to_i16_vec(slice));
            }
            break;
        }
        
        total_bytes_in_buffer += bytes_read;

        // If the buffer is full, process it and reset the buffer
        if total_bytes_in_buffer == buffer.len() {
            f(convert_to_i16_vec(&buffer));
            total_bytes_in_buffer = 0; // Reset the buffer
        }
    }

    // Wait for the child process to finish
    let status = ffmpeg_process.wait().await?;
    if !status.success() {
        return Err(format!("ffmpeg failed with a non-zero exit code {}", status.code().unwrap_or(-1)).into());
    }

    Ok(())
}


pub async fn streaming_url<F>(input_url: &str, target_sample_rate: i64, sample_size: usize,mut f: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(Vec<i16>),
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
        .output()
        .await?;


    // Convert the output to a string
    let stdout = str::from_utf8(&output.stdout).expect("Invalid UTF-8 sequence");

    // Parse the JSON output
    let ffprobe_output: FFProbeOutput = serde_json::from_str(stdout).expect("Failed to parse JSON");


        // Check if duration exists and print it
        if let Some(duration) = ffprobe_output.format.duration {
            eprintln!("Duration: {} seconds", duration);
            streaming_inner_loop(input_url, target_sample_rate, sample_size, &mut f).await?;
        } else {
            eprintln!("No duration found, assuming stream is infinite and will restart on stream stop");
            loop {
                streaming_inner_loop(input_url, target_sample_rate, sample_size, &mut f).await?;
                eprintln!("stream_stopped, restarting");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

    Ok(())
}
