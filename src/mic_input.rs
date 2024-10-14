//! Records a WAV file (roughly 3 seconds long) using the default input device and config.
//!
//! The input data is recorded to "$CARGO_MANIFEST_DIR/recorded.wav".

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, StreamConfig, SupportedStreamConfig, SupportedStreamConfigRange};
use crossbeam::channel::Sender;
use ort::Output;
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use dasp_sample::{Sample, ToSample};

use crate::sample;

pub fn record(tx: &'static Sender<Vec<i16>>,sample_size: usize) -> Result<(), Box<dyn std::error::Error>> {
    // Conditionally compile with jack if the feature is specified.
    #[cfg(all(
        any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd"
        ),
        feature = "jack"
    ))]
    // Manually check for flags. Can be passed through cargo with -- e.g.
    // cargo run --release --example beep --features jack -- --jack
    let host = if opt.jack {
        cpal::host_from_id(cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
            .expect(
                "make sure --features jack is specified. only works on OSes where jack is available",
            )).expect("jack host unavailable")
    } else {
        cpal::default_host()
    };

    #[cfg(any(
        not(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd"
        )),
        not(feature = "jack")
    ))]
    let host = cpal::default_host();

    // Set up the input device and stream with the default input config.
    let device = host.default_input_device().expect("Failed to get default input device");

    println!("Input device: {}", device.name()?);


    let config = StreamConfig {
         channels: 1,
         sample_rate: cpal::SampleRate(16000), 
         buffer_size: BufferSize::Fixed(sample_size as u32) };
    
    //eprintln!("Default input config: {:?}", config);

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| write_input_data::<f32>(data,&tx),
            err_fn,
            None,
        )?;


    stream.play()?;
    loop {
        sleep(Duration::from_secs(10));
        eprintln!("recording looping still");
    }
    Ok(())
}

fn write_input_data<T>(input: &[T], tx: &Sender<Vec<i16>>)
where
    T: Sample + ToSample<i16> + Copy,
{
    eprintln!("input len: {}", input.len());

    
    let output: Vec<i16> = input.iter().map(|&x| x.to_sample::<i16>()).collect::<Vec<i16>>();
        
    tx.send(output).unwrap();
    
}