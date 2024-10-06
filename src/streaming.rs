use std::convert::TryInto;
use tokio::process::{ChildStdout, Command};
use tokio_stream::StreamExt;
use tokio_util::{bytes::Bytes, io::ReaderStream};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};


pub async fn streaming_url<F>(input_url: &str, target_sample_rate: i32, mut f: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(Vec<u8>),
{
    // Path to the input file
    //let input_file = "input.mp3"; // Replace with your file path

    // Run ffmpeg to get raw PCM (s16le) data at 16kHz
    let mut ffmpeg_process = Command::new("ffmpeg")
        .args(&[
            "-i", input_url,      // Input url
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
    


    loop {
        // 16 kHz * 2 bytes per sample * 1 channels
        let one_second: usize = (target_sample_rate * 2 * 1).try_into().unwrap(); 
        // Buffer for reading 16,000 bytes
        let mut buffer = vec![0u8; one_second]; 

        let n = reader.read_exact(&mut buffer).await?;
        
        // Process the chunk (n is the number of bytes read)
        if n == 0 {
            break;
        }

        // Call the closure with the chunk
        f(buffer);
    }

    // Wait for the child process to finish
    let status = ffmpeg_process.wait().await?;
    if !status.success() {
        return Err(format!("ffmpeg failed with a non-zero exit code {}", status.code().unwrap_or(-1)).into());
    }

    Ok(())
}
