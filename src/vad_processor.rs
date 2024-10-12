use hound::{self};
use reqwest::blocking::get;
use sha1::{Sha1, Digest};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite, SqlitePool};
use url::Url;
use ringbuffer::{AllocRingBuffer, RingBuffer};

use crate::{config::Config, streaming::streaming_url, vad::VoiceActivityDetector};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use std::io;
use std::{fs::{self}, path::Path, time::{SystemTime, UNIX_EPOCH}};
use serde_json::json;

use zhconv::{zhconv, Variant};

use std::thread::available_parallelism;

enum State {
    NoSpeech,
    HasSpeech,
}

impl State {
    fn convert(has_speech: bool) -> State {
        match has_speech {
            true => State::HasSpeech,
            _ => State::NoSpeech,
        }
    }
}

const TARGET_SAMPLE_RATE: i64 = 16000;
const SAMPLE_SIZE: usize = 1024;

   
/*
The VAD predicts speech in a chunk of Linear Pulse Code Modulation (LPCM) encoded audio samples. These may be 8 or 16 bit integers or 32 bit floats.

The model is trained using chunk sizes of 256, 512, and 768 samples for an 8000 hz sample rate. It is trained using chunk sizes of 512, 768, 1024 samples for a 16,000 hz sample rate.
*/
 
fn process_buffer_with_vad<F>(model: &mut VoiceActivityDetector,url: &str, mut f: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(&Vec<i16>),
{
    //let target_sample_rate: i32 = 16000;


    let mut buf:Vec<i16> = Vec::new();
    //let mut num = 1;

    let min_speech_duration_seconds = 3.0;

    let mut has_speech = false;

    //let mut prev_sample:Option<Vec<i16>> = None;

    //let mut has_speech_time = 0.0;

    let mut prev_state = State::NoSpeech;

    //let prev_size = ;

    // one second
    let mut prev_samples = AllocRingBuffer::<i16>::new(TARGET_SAMPLE_RATE as usize);


    //let whisper_wrapper_ref = RefCell::new(whisper_wrapper);
    //let whisper_wrapper_ref2 = &whisper_wrapper;
    let closure_annotated = |samples: Vec<i16>| {
        eprintln!("Received sample size: {}", samples.len());
        //assert!(samples.len() as i32 == target_sample_rate); //make sure it is one second
        //let sample2 = samples.clone();
        //silero.reset();
        //let mut rng = rand::thread_rng();
        //let probability: f64 = rng.gen();
        let probability = model.predict(samples.clone());
        //let len_after_samples: i32 = (buf.len() + samples.len()).try_into().unwrap();
        eprintln!("buf.len() {}", buf.len());
        let seconds = buf.len() as f32 / TARGET_SAMPLE_RATE as f32;
        //eprintln!("len_after_samples / target_sample_rate {}",seconds);

        if probability > 0.5 {
            eprintln!("Chunk is speech: {}", probability);
            has_speech = true;
        } else {
            has_speech = false;
        }

        assert!(prev_samples.len()<=TARGET_SAMPLE_RATE as usize);
        match prev_state {
            State::NoSpeech => {
                if has_speech {
                    eprintln!("Transitioning from no speech to speech");
                    // add previous sample if it exists
                    //if let Some(prev_sample2) = &prev_sample {
                    if prev_samples.len() > 0 {
                        buf.extend(&prev_samples);
                        prev_samples.clear();
                        //std::process::exit(1)
                    }
                        assert_eq!(prev_samples.len(),0);
                    //}
                    // start to extend the buffer
                    buf.extend(&samples);
                } else {
                    eprintln!("Still No Speech");
                    prev_samples.extend(samples.iter().cloned());
                }
            },
            State::HasSpeech => {
                if seconds < min_speech_duration_seconds {
                    eprintln!("override to Continue to has speech because seconds < min_seconds {}", seconds);
                    has_speech = true;
                }
                if has_speech {
                    eprintln!("Continue to has speech");
                    // continue to extend the buffer
                    buf.extend(&samples);
                } else {
                    eprintln!("Transitioning from speech to no speech");
                    buf.extend(&samples);
                    //save the buffer if not empty
                    f(&buf);
                    buf.clear();
                    prev_samples.clear();
                }
            }
        }

        prev_state = State::convert(has_speech);
        

    };

    streaming_url(url,TARGET_SAMPLE_RATE,SAMPLE_SIZE,closure_annotated)?;

    if buf.len() > 0 {
        f(&buf);
        buf.clear();
        //num += 1;
    }


    Ok(())
}




fn sync_buf_to_file(buf: &Vec<i16>, file_name: &str) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(file_name, spec).unwrap();
    for sample in buf {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
}


fn transcribe(state: &mut WhisperState, params: &whisper_rs::FullParams, samples: &Vec<i16>) {
    
    // Create a state
    let mut audio = vec![0.0f32; samples.len().try_into().unwrap()];

    whisper_rs::convert_integer_to_float_audio(&samples, &mut audio).expect("Conversion error");

    // Run the model.
    state.full(params.clone(), &audio[..]).expect("failed to run model");

    //eprintln!("{}",state.full_n_segments().expect("failed to get number of segments"));
    //samples.clear();
}

fn get_filename_from_url(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    
    // Parse the URL
    let parsed_url = Url::parse(url)?;
    let path = parsed_url.path();
    if let Some(filename) = Path::new(path).file_name(){
        Ok(filename.to_str().unwrap().to_string())
    }else{
        Err("Failed to get filename from URL".into())
    }
}

fn download_to_temp_and_move(url: &str, destination: &str) -> Result<(), Box<dyn std::error::Error>> {

    // Create a temporary file. This file will be automatically deleted when dropped.
    let mut temp_file = tempfile::tempfile()?;

    // Download the file
    let mut response = get(url)?;
    
    if response.status().is_success() {
        io::copy(&mut response, &mut temp_file)?;
        //while let Some(chunk) = response.chunk().await? {
        //    temp_file.write_all(&chunk)?;
        //}
        //// Move the temp file to the destination only if the download was successful.
        //temp_file.persist(destination)?; // Moves the file to the final destination
        eprintln!("File downloaded and moved to: {}", destination);
    } else {
        println!("Failed to download the file. Status: {}", response.status());
    }

    drop(temp_file);

    Ok(())
}

fn get_vad() -> VoiceActivityDetector{

    let v4_download_url = "https://github.com/snakers4/silero-vad/raw/refs/tags/v4.0/files/silero_vad.onnx";

    //let v5_download_url = "https://github.com/snakers4/silero-vad/raw/refs/tags/v5.1.2/src/silero_vad/data/silero_vad.onnx";

    //let half = "https://github.com/snakers4/silero-vad/raw/refs/tags/v5.1.2/src/silero_vad/data/silero_vad_half.onnx";

    let download_url = v4_download_url;

    // Create a Sha1 object
    let mut hasher = Sha1::new();

    // Write the input string to the hasher
    hasher.update(download_url);

    // Get the resulting hash as a hexadecimal string
    let result = hasher.finalize();

    // Convert the hash to a hex string
    let hex_output = format!("{:x}", result);

    let model_local_directory = dirs::cache_dir().unwrap().join(hex_output).join("whisper_transcribe_rs");
    fs::create_dir_all(&model_local_directory).unwrap();
    let file_name = get_filename_from_url(download_url).unwrap();
    let model_path = model_local_directory.join(file_name);
    if !model_path.exists() {
        eprintln!("Downloading model from {} to {}", download_url, model_path.to_str().unwrap());
        download_to_temp_and_move(download_url, model_path.to_str().unwrap()).unwrap();
    }

    VoiceActivityDetector::build(TARGET_SAMPLE_RATE,SAMPLE_SIZE,&model_path)
}

pub fn stream_to_file(config: Config) -> Result<(), Box<dyn std::error::Error>>{
    //let url = "https://rthkradio2-live.akamaized.net/hls/live/2040078/radio2/master.m3u8";
    //let url = "https://www.am1430.net/wp-content/uploads/show/%E7%B9%BC%E7%BA%8C%E6%9C%89%E5%BF%83%E4%BA%BA/2023/2024-10-03.mp3";
    //println!("First argument: {}", first_argument);

    let url = config.url.as_str();

    let mut num = 1;
    let closure_annotated = |buf: &Vec<i16>| {
        let file_name = format!("tmp/predict.stream.speech.{}.wav", format!("{:0>3}",num));
        sync_buf_to_file(&buf, &file_name);
        num += 1;
    };

    let mut model = get_vad();

    process_buffer_with_vad(&mut model,url,closure_annotated)?;

    Ok(())
}

pub fn transcribe_url(config: Config,num_transcribe_threads: Option<usize>,model_download_url: &str) -> Result<(), Box<dyn std::error::Error>> {
 
    let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all().build()?;

    let url = config.url.as_str();
    let mut pool: Option<Pool<Sqlite>> = None;

    if let Some(database_file_path) = &config.database_file_path {
        pool = rt.block_on(async {
            let pool2 = SqlitePool::connect_with(SqliteConnectOptions::new().filename(database_file_path)
                .create_if_missing(true)).await.unwrap();
            //let conn2 = SqliteConnection::connect(database_file_path).await?;
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS transcripts (
                        id INTEGER PRIMARY KEY,
                        timestamp datetime NOT NULL,
                        content TEXT NOT NULL
                )").execute(&pool2).await.unwrap();
            Some(pool2)
        });
         //pool = Some(pool2);
    }


    // Load a context and model.
    let context_param = WhisperContextParameters::default();

    let download_url = model_download_url;

    let model_local_directory = dirs::cache_dir().unwrap().join("whisper_transcribe_rs");
    fs::create_dir_all(&model_local_directory).unwrap();
    let file_name = get_filename_from_url(download_url).unwrap();
    let model_path = model_local_directory.join(file_name);
    if !model_path.exists() {
        eprintln!("Downloading model from {} to {}", download_url, model_path.to_str().unwrap());
        download_to_temp_and_move(download_url, model_path.to_str().unwrap()).unwrap();
    }

    let ctx = WhisperContext::new_with_params(
        model_path.to_str().unwrap(),
        context_param,
    ).expect("failed to load model");



    // Create a params object for running the model.
    // The number of past samples to consider defaults to 0.
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });


    let n_threads = match num_transcribe_threads {
        Some(n) => n,
        None => {
            // get 4 or the number of cpus if less than 4
            let default_parallelism_approx = available_parallelism().unwrap().get();
            *[default_parallelism_approx,4].iter().min().unwrap_or(&1)
        }
    };


    //assert_eq!(n_threads,1);

    // Edit params as needed.
    // Set the number of threads to use to 4.
    params.set_n_threads(n_threads as i32);
    // Enable translation.
    params.set_translate(false);
    // Set the language to translate to to English.
    params.set_language(Some(config.language.as_str()));
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

    let language = config.language.clone();
    params.set_segment_callback_safe( move |data: whisper_rs::SegmentCallbackData| {

        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        let line = json!({"start_timestamp":data.start_timestamp,
            "end_timestamp":data.end_timestamp, "cur_ts": since_the_epoch.as_millis() as f64/1000.0, "text":data.text});
        println!("{}", line);

        // only convert to traditional chinese when saving to db
        // output original in jsonl
        let db_save_text = match language.as_str() {
            "zh" | "yue" => {
                zhconv(&data.text, Variant::ZhHant)
            },
            _ => {
                data.text
            }
        };

        rt.block_on(async {
            if let Some(pool) = &pool {
                sqlx::query(
                    "INSERT INTO transcripts (timestamp, content) VALUES (?, ?)",
                ).bind(since_the_epoch.as_millis() as f64/1000.0)
                .bind(db_save_text)
                .execute(pool).await.unwrap();
            }
        });

    
    });


    let mut state = ctx.create_state().expect("failed to create key");


    //let whisper_wrapper_ref = RefCell::new(whisper_wrapper);
    //let whisper_wrapper_ref2 = &whisper_wrapper;
    let closure_annotated = |buf: &Vec<i16>| {

            transcribe(&mut state, &params, &buf);

    };

    let mut model = get_vad();

    process_buffer_with_vad(&mut model,url,closure_annotated)?;
        
    Ok(())
}