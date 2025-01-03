//! Records a WAV file (roughly 3 seconds long) using the default input device and config.
//!
//! The input data is recorded to "$CARGO_MANIFEST_DIR/recorded.wav".

use chrono::{TimeZone, Utc};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleRate, StreamConfig};
use crossbeam::channel::Sender;
use dasp_sample::{Sample};
use log::{error, trace};
use samplerate::{convert, ConverterType};
use console::Term;

use crate::streaming::Segment;

pub fn record_from_mic(tx: &Sender<Option<Segment>>,sample_size: usize) -> Result<(), Box<dyn std::error::Error>> {
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
    
    //debug!("Default input config: {:?}", config);

    let err_fn = move |err| {
        error!("an error occurred on stream: {}", err);
    };

    let tx2 = tx.clone();

    let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| {
                let tx2 = tx2.clone();
                write_input_data(data,sample_rate,tx2);
            },
            err_fn,
            None,
        )?;


    stream.play()?;

    println!("Press 'q' to quit:");

    let stdout = Term::buffered_stdout();

    loop {
        let char = stdout.read_char()?;

        if char == 'q' {
            println!("Exiting...");
            break;
        }

        //println!("You entered: {}", char);
    }
    tx.send(None)?;
    Ok(())
}


fn write_input_data(input: &[f32],sample_rate: SampleRate, tx: Sender<Option<Segment>>)
{
    let resampled: Vec<f32>= if sample_rate.0 > 16000 {
    // Resample the input to 16000hz.
      convert(sample_rate.0, 16000, 1, ConverterType::SincBestQuality, input).unwrap()
    }else{
        input.to_vec()
    };
    
    let output: Vec<i16> = resampled.iter().map(|&x| x.to_sample::<i16>()).collect::<Vec<i16>>();
    trace!("output len: {}", output.len());
    let timestamp_millis = Utc::now().timestamp_millis();
    tx.send(Some(Segment{timestamp_millis, samples: output})).unwrap();
    
}