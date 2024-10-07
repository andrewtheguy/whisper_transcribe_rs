use hound::{self, Sample};

use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::json;
use tokio_stream::{self, StreamExt};
use whisper_rs_test::vad_processor::process_buffer_with_vad;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use rusqlite::{params, Connection, Result};

fn transcribe(state: &mut WhisperState, params: &whisper_rs::FullParams, samples: &Vec<i16>) {

    // Create a state
    let mut audio = vec![0.0f32; samples.len().try_into().unwrap()];

    whisper_rs::convert_integer_to_float_audio(&samples, &mut audio).expect("Conversion error");

    // Run the model.
    state.full(params.clone(), &audio[..]).expect("failed to run model");

    //eprintln!("{}",state.full_n_segments().expect("failed to get number of segments"));
    //samples.clear();
}

/*
The VAD predicts speech in a chunk of Linear Pulse Code Modulation (LPCM) encoded audio samples. These may be 8 or 16 bit integers or 32 bit floats.

The model is trained using chunk sizes of 256, 512, and 768 samples for an 8000 hz sample rate. It is trained using chunk sizes of 512, 768, 1024 samples for a 16,000 hz sample rate.
*/
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open a connection to the SQLite database (or create it if it doesn't exist)
    let conn = Connection::open("./tmp/example.db")?;

    // Create a table with a timestamp and text column
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transcripts (
                  id INTEGER PRIMARY KEY,
                  timestamp TEXT NOT NULL,
                  content TEXT NOT NULL
          )",
        [],
    )?;


    let target_sample_rate = 16000;
    let sample_size: usize = 1024;

    let url = "https://rthkradio2-live.akamaized.net/hls/live/2040078/radio2/master.m3u8";
    //let url = "https://www.am1430.net/wp-content/uploads/show/%E7%B9%BC%E7%BA%8C%E6%9C%89%E5%BF%83%E4%BA%BA/2023/2024-10-03.mp3";
    //println!("First argument: {}", first_argument);


    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();

    whisper_rs::install_whisper_log_trampoline();

    // Load a context and model.
    let context_param = WhisperContextParameters::default();

    let ctx = WhisperContext::new_with_params(
        "/Users/it3/codes/andrew/transcribe_audio/whisper_models/ggml-large-v3-turbo.bin",
        context_param,
    ).expect("failed to load model");



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
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let line = json!({"start_timestamp":data.start_timestamp,
            "end_timestamp":data.end_timestamp, "cur_ts": since_the_epoch.as_millis() as f64/1000.0, "text":data.text});
        println!("{}", line);

        conn.execute(
            "INSERT INTO transcripts (timestamp, content) VALUES (?1, ?2)",
            params![since_the_epoch.as_millis() as f64/1000.0, data.text],
        ).unwrap();
    
    });


    let mut state = ctx.create_state().expect("failed to create key");


    //let whisper_wrapper_ref = RefCell::new(whisper_wrapper);
    //let whisper_wrapper_ref2 = &whisper_wrapper;
    let closure_annotated = |buf: &Vec<i16>| {

            transcribe(&mut state, &params.clone(), &buf);

    };

    process_buffer_with_vad(url,target_sample_rate,sample_size,closure_annotated).await?;



    Ok(())
}
