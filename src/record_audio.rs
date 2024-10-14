//! Records a WAV file (roughly 3 seconds long) using the default input device and config.
//!
//! The input data is recorded to "$CARGO_MANIFEST_DIR/recorded.wav".

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleRate, StreamConfig, SupportedStreamConfig, SupportedStreamConfigRange};
use crossbeam::channel::Sender;
use ort::Output;
use tokio_util::bytes::buf;
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use dasp_sample::{Sample, ToSample};

use samplerate::{convert, ConverterType};

pub fn record_from_mic(tx: &'static Sender<Vec<i16>>,sample_size: usize) -> Result<(), Box<dyn std::error::Error>> {
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

    let mut supported = device.supported_input_configs()?;
    let supported_config = if let Some(config) = supported.next() {
        println!("supported input config: {:?}", &config);
        config
    } else {
        return Err("Failed to get supported input config".into());
    };

    if supported_config.max_sample_rate() < cpal::SampleRate(16000) {
        return Err("max sample rate is less than 16000".into());
    }
    
    let mut sample_rate = supported_config.min_sample_rate();

    // just hack it with a number that divides evenly with 16000
    if sample_rate.0 > 16000 {
        sample_rate = cpal::SampleRate(48000);
    }

    let buffer_size = sample_rate.0/16000 * sample_size as u32;
    
    let config = StreamConfig {
         channels: 1,
         sample_rate: sample_rate, 
         buffer_size: BufferSize::Fixed(buffer_size) };
    
    //eprintln!("Default input config: {:?}", config);

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| write_input_data(data,sample_rate,&tx),
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


pub fn record_computer_output(tx: &'static Sender<Vec<i16>>,sample_size: usize) -> Result<(), Box<dyn std::error::Error>> {
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
    let device = host.default_output_device().expect("Failed to get default output device");

    println!("Output device: {}", device.name()?);

    let mut supported = device.supported_output_configs()?;
    let supported_config = if let Some(config) = supported.next() {
        println!("supported output config: {:?}", &config);
        config
    } else {
        return Err("Failed to get supported output config".into());
    };

    if supported_config.max_sample_rate() < cpal::SampleRate(16000) {
        return Err("max sample rate is less than 16000".into());
    }
    
    let mut sample_rate = supported_config.min_sample_rate();

    // just hack it with a number that divides evenly with 16000
    if sample_rate.0 > 16000 {
        sample_rate = cpal::SampleRate(48000);
    }

    let buffer_size = sample_rate.0/16000 * sample_size as u32;
    
    let config = StreamConfig {
         channels: 1,
         sample_rate: sample_rate, 
         buffer_size: BufferSize::Fixed(buffer_size) };
    
    //eprintln!("Default input config: {:?}", config);

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    let stream = device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                write_input_data(data,sample_rate,&tx)
            },
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


fn write_input_data(input: &[f32],sample_rate: SampleRate, tx: &Sender<Vec<i16>>)
{
    let resampled: Vec<f32>= if sample_rate.0 > 16000 {
    // Resample the input to 16000hz.
      convert(sample_rate.0, 16000, 1, ConverterType::SincBestQuality, input).unwrap()
    }else{
        input.to_vec()
    };
    
    let output: Vec<i16> = resampled.iter().map(|&x| x.to_sample::<i16>()).collect::<Vec<i16>>();
    eprintln!("output len: {}", output.len());
    tx.send(output).unwrap();
    
}