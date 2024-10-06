use byteorder::{ByteOrder, LittleEndian};
use hound::{self, Sample};

use log4rs::append::file;
use serde_json::json;
use voice_activity_detector::{StreamExt as _, VoiceActivityDetector};
use tokio_stream::{self, StreamExt};
use tokio::io::{self, BufReader};
use tokio_util::{bytes::buf, io::ReaderStream};
use whisper_rs_test::streaming::streaming_url;
use tokio_util::{bytes::Bytes};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Debug, PartialEq, Eq)]
enum SpeechStatus {
    Speech,
    NonSpeech,
}

fn convert_to_i16_vec(buf: &[u8]) -> Vec<i16> {
    let mut vec = Vec::with_capacity(buf.len() / 2); // Allocate space for i16 values
    for chunk in buf.chunks_exact(2) {
        vec.push(LittleEndian::read_i16(chunk));
    }
    vec
}

fn sync_buf_to_file(buf: &mut Vec<i16>, file_name: &str) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(file_name, spec).unwrap();
    for sample in &mut *buf {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
    buf.clear();
}

fn transcribe(ctx: &WhisperContext, params: &whisper_rs::FullParams, samples: &Vec<i16>) {

    // Create a state
    let mut state = ctx.create_state().expect("failed to create key");

    let mut audio = vec![0.0f32; samples.len().try_into().unwrap()];

    whisper_rs::convert_integer_to_float_audio(&samples, &mut audio).expect("Conversion error");

    // Run the model.
    state.full(params.clone(), &audio[..]).expect("failed to run model");

    //eprintln!("{}",state.full_n_segments().expect("failed to get number of segments"));



}

/*
The VAD predicts speech in a chunk of Linear Pulse Code Modulation (LPCM) encoded audio samples. These may be 8 or 16 bit integers or 32 bit floats.

The model is trained using chunk sizes of 256, 512, and 768 samples for an 8000 hz sample rate. It is trained using chunk sizes of 512, 768, 1024 samples for a 16,000 hz sample rate.
*/
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_sample_rate: i32 = 16000;

    let url = "https://rthkradio2-live.akamaized.net/hls/live/2040078/radio2/master.m3u8";
    //let url = "https://www.am1430.net/wp-content/uploads/show/%E7%B9%BC%E7%BA%8C%E6%9C%89%E5%BF%83%E4%BA%BA/2023/2024-10-03.mp3";
    //println!("First argument: {}", first_argument);



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


    //let samples = [0i16; 51200];
    let mut vad = VoiceActivityDetector::builder()
        .sample_rate(target_sample_rate)
        .chunk_size(512usize)
        .build()?;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: target_sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    // let mut nonspeech =
    //     hound::WavWriter::create("tmp/predict.stream.nonspeech.wav", spec)?;

    let mut buf:Vec<i16> = Vec::new();    
    let mut num = 1;
    let max_seconds = 10;
    //let size_for_one_second = target_sample_rate * 2;
    //let cur_seconds = 0;
    let closure_annotated = |chunk: Vec<u8>| {
        eprintln!("Received chunk of size: {}", chunk.len());
        //assert!(chunk.len() as i32 == target_sample_rate * 2); //make sure it is one second
        //cur_seconds += 1;
        let samples = convert_to_i16_vec(&chunk);
        //assert!(samples.len() as i32 == target_sample_rate); //make sure it is one second
        let probability = vad.predict(samples.clone());
        let len_after_samples: i32 = (buf.len() + samples.len()).try_into().unwrap();
        if buf.len() > 0 && (len_after_samples / target_sample_rate) % max_seconds == 0 {
            eprintln!("Chunk is more than {} seconds, flushing", max_seconds);
            buf.extend(&samples);
            transcribe(&ctx, &params, &buf);
            //let file_name = format!("tmp/predict.stream.speech.{}.wav", num);
            //sync_buf_to_file(&mut buf, &file_name);
            num += 1;
            //cur_seconds = 0;
        } else if probability > 0.5 {
            eprintln!("Chunk is speech: {}", probability);
            buf.extend(&samples);
        } else {
            eprintln!("Chunk is not speech: {}", probability);
            if buf.len() > 0 {
                buf.extend(&samples);
                transcribe(&ctx, &params, &buf);
                //let file_name = format!("tmp/predict.stream.speech.{}.wav", num);
                //sync_buf_to_file(&mut buf, &file_name);
                num += 1;
            }
        }
    };

    streaming_url(url,target_sample_rate,closure_annotated).await?;

    if buf.len() > 0 {
        transcribe(&ctx,  &params,  &buf);
        //let file_name = format!("tmp/predict.stream.speech.noend.wav");
        //sync_buf_to_file(&mut buf, &file_name);
        num += 1;
    }


    Ok(())
}
