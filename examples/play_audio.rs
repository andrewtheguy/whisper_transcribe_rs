use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, StreamConfig};
use std::fs::File;
use std::io::{Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let (tx, rx) = std::sync::mpsc::channel::<f32>();

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
    let stream = output_device.build_output_stream(
        &desired_config,
        move |data: &mut [f32], _| {
            for sample in data {
                *sample = match rx.recv() {
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

    // Start the stream
    stream.play()?;

    

    // Open the PCM audio file
    let mut pcm_reader = File::open("tmp/output_file.pcm").expect("Failed to open PCM file");
    
    let chunk_size = 1024;
    let mut buffer = vec![0; chunk_size]; // Create a buffer to hold the chunk

    loop {
        let bytes_read = pcm_reader.read(&mut buffer)?; // Read into the buffer
        if bytes_read == 0 {
            break; // End of file reached
        }
        
        // Process the chunk here
        // For example, converting bytes to string (if the data is valid UTF-8)
        let chunk = &buffer[..bytes_read];
        
            // Process each pair of bytes as a 16-bit sample
            for i in 0..bytes_read / 2 {
                // Combine two bytes into a single 16-bit sample
                let sample_bytes: [u8; 2] = [
                    chunk[i * 2] as u8,         // Low byte
                    chunk[i * 2 + 1] as u8,     // High byte
                ];
                let sample = i16::from_le_bytes(sample_bytes); // Convert to i16
                

                let sample: f32 = f32::from_sample(sample);
                tx.send(sample)?;
            }
    }
    // Keep the main thread alive to allow audio to play
    std::thread::sleep(std::time::Duration::from_secs(60));

    Ok(())
}
