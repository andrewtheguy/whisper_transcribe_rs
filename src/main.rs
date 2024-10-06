// This example is not going to build in this folder.
// You need to copy this code into your project and add the dependencies whisper_rs and hound in your cargo.toml

use hound::{self, Sample};
use serde_json::json;
use std::{fs::File, process, env};
use std::io::Write;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use std::process::{Command, Stdio};
use std::io::{Read, Cursor};
use std::convert::TryInto;

use whisper_rs_test::convert_file_to_wave;

// fn old() {

//     // Open the audio file.
//     let reader = hound::WavReader::open(input_file).expect("failed to open file");
//     #[allow(unused_variables)]
//     let hound::WavSpec {
//         channels,
//         sample_rate,
//         bits_per_sample,
//         ..
//     } = reader.spec();

//     // Convert the audio to floating point samples.
//     let samples: Vec<i16> = reader
//         .into_samples::<i16>()
//         .map(|x| x.expect("Invalid sample"))
//         .collect();
//     let mut audio = vec![0.0f32; samples.len().try_into().unwrap()];
//     whisper_rs::convert_integer_to_float_audio(&samples, &mut audio).expect("Conversion error");

//     // Convert audio to 16KHz mono f32 samples, as required by the model.
//     // These utilities are provided for convenience, but can be replaced with custom conversion logic.
//     // SIMD variants of these functions are also available on nightly Rust (see the docs).
//     if channels == 2 {
//         audio = whisper_rs::convert_stereo_to_mono_audio(&audio).expect("Conversion error");
//     } else if channels != 1 {
//         panic!(">2 channels unsupported");
//     }

//     if sample_rate != 16000 {
//         panic!("sample rate must be 16KHz");
//     }

// }


/// Loads a context and model, processes an audio file, and prints the resulting transcript to stdout.
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

    let mut audio = vec![0.0f32; samples.len().try_into().unwrap()];

    whisper_rs::convert_integer_to_float_audio(&samples, &mut audio).expect("Conversion error");


    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();

    whisper_rs::install_whisper_log_trampoline();
    
    // Load a context and model.
    let context_param = WhisperContextParameters::default();

    // // Enable DTW token level timestamp for known model by using model preset
    // context_param.dtw_parameters.mode = whisper_rs::DtwMode::ModelPreset {
    //     model_preset: whisper_rs::DtwModelPreset::BaseEn,
    // };

    // // Enable DTW token level timestamp for unknown model by providing custom aheads
    // // see details https://github.com/ggerganov/whisper.cpp/pull/1485#discussion_r1519681143
    // // values corresponds to ggml-base.en.bin, result will be the same as with DtwModelPreset::BaseEn
    // let custom_aheads = [
    //     (3, 1),
    //     (4, 2),
    //     (4, 3),
    //     (4, 7),
    //     (5, 1),
    //     (5, 2),
    //     (5, 4),
    //     (5, 6),
    // ]
    // .map(|(n_text_layer, n_head)| whisper_rs::DtwAhead {
    //     n_text_layer,
    //     n_head,
    // });
    // context_param.dtw_parameters.mode = whisper_rs::DtwMode::Custom {
    //     aheads: &custom_aheads,
    // };

    let ctx = WhisperContext::new_with_params(
        "/Users/it3/codes/andrew/transcribe_audio/whisper_models/ggml-large-v3-turbo.bin",
        context_param,
    )
    .expect("failed to load model");
    // Create a state
    let mut state = ctx.create_state().expect("failed to create key");

    // Create a params object for running the model.
    // The number of past samples to consider defaults to 0.
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });

    // Edit params as needed.
    // Set the number of threads to use to 4.
    params.set_n_threads(4);
    // Enable translation.
    params.set_translate(false);
    // Set the language to translate to to English.
    params.set_language(Some("yue"));
    // Disable anything that prints to stdout.
    params.set_print_special(false);
    params.set_debug_mode(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    // Enable token level timestamps
    params.set_token_timestamps(true);
    params.set_n_max_text_ctx(64);

    //let mut file = File::create("transcript.jsonl").expect("failed to create file");

    params.set_segment_callback_safe( move |data: whisper_rs::SegmentCallbackData| {
        let line = json!({"start_timestamp":data.start_timestamp, 
        "end_timestamp":data.end_timestamp, "text":data.text});
        println!("{}", line);
        //writeln!(file, "{}", line).expect("failed to write to file");
    });

    // Run the model.
    state.full(params, &audio[..]).expect("failed to run model");

    eprintln!("{}",state.full_n_segments().expect("failed to get number of segments"));

    // // Create a file to write the transcript to.
    // let mut file = File::create("transcript.txt").expect("failed to create file");

    // // Iterate through the segments of the transcript.
    // let num_segments = state
    //     .full_n_segments()
    //     .expect("failed to get number of segments");
    // for i in 0..num_segments {
    //     // Get the transcribed text and timestamps for the current segment.
    //     let segment = state
    //         .full_get_segment_text(i)
    //         .expect("failed to get segment");
    //     let start_timestamp = state
    //         .full_get_segment_t0(i)
    //         .expect("failed to get start timestamp");
    //     let end_timestamp = state
    //         .full_get_segment_t1(i)
    //         .expect("failed to get end timestamp");

    //     // let first_token_dtw_ts = if let Ok(token_count) = state.full_n_tokens(i) {
    //     //     if token_count > 0 {
    //     //         if let Ok(token_data) = state.full_get_token_data(i, 0) {
    //     //             token_data.t_dtw
    //     //         } else {
    //     //             -1i64
    //     //         }
    //     //     } else {
    //     //         -1i64
    //     //     }
    //     // } else {
    //     //     -1i64
    //     // };
    //     // Print the segment to stdout.
    //     println!(
    //         "[{} - {} ({})]: {}",
    //         start_timestamp, end_timestamp, first_token_dtw_ts, segment
    //     );

    //     // Format the segment information as a string.
    //     let line = json!({"start_timestamp":start_timestamp, 
    //     "end_timestamp":end_timestamp, "text":segment});

    //     // Write the segment information to the file.
    //     writeln!(file, "{}", line).expect("failed to write to file");
    // }
    
    Ok(())
}
