use crossbeam::channel::{bounded, unbounded, Receiver};
use hound::{self};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
//use sqlx::sqlite::{SqliteConnectOptions};
use sqlx::Pool;
use ringbuffer::{AllocRingBuffer, RingBuffer};

use crate::download_utils::{get_whisper_model, get_silero_model};
use crate::key_ring_utils::get_password;
use crate::sample;
use crate::{config::Config, streaming::streaming_url, vad::VoiceActivityDetector};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use core::panic;
use std::thread;
use serde_json::json;

use zhconv::{zhconv, Variant};

use std::thread::available_parallelism;

use chrono::Utc;

use crate::utils::build_current_thread_runtime;

use std::sync::LazyLock;

use crate::record_audio::{record_from_mic};

struct ChannelPair {
    tx: crossbeam::channel::Sender<Option<Vec<i16>>>,
    rx: crossbeam::channel::Receiver<Option<Vec<i16>>>,
}

// n.b. static items do not call [`Drop`] on program termination, so this won't be deallocated.
// this is fine, as the OS can deallocate the terminated program faster than we can free memory
// but tools like valgrind might report "memory leaks" as it isn't obvious this is intentional.
static MIC_CHANNEL_PAIR: LazyLock<ChannelPair> = LazyLock::new(|| {
    let (tx, rx) = unbounded::<Option<Vec<i16>>>().try_into().unwrap();
    ChannelPair{tx,rx}
});


enum SpeechTag {
    NoSpeech,
    HasSpeech,
}

impl SpeechTag {
    fn convert(has_speech: bool) -> SpeechTag {
        match has_speech {
            true => SpeechTag::HasSpeech,
            _ => SpeechTag::NoSpeech,
        }
    }
}

const TARGET_SAMPLE_RATE: i64 = 16000;
const SAMPLE_SIZE: usize = 1024;


/*
The VAD predicts speech in a chunk of Linear Pulse Code Modulation (LPCM) encoded audio samples. These may be 8 or 16 bit integers or 32 bit floats.

The model is trained using chunk sizes of 256, 512, and 768 samples for an 8000 hz sample rate. It is trained using chunk sizes of 512, 768, 1024 samples for a 16,000 hz sample rate.
*/

fn process_buffer_with_vad<E,F>(rx: &Receiver<Option<Vec<i16>>>, input_callback: E, mut output_callback: F) -> Result<(), Box<dyn std::error::Error>>
where
    E: FnOnce() + std::marker::Send,
    F: FnMut(&Vec<i16>) + std::marker::Send,
{
    //let target_sample_rate: i32 = 16000;

    //let (tx, rx) = bounded::<Vec<i16>>((TARGET_SAMPLE_RATE*60).try_into().unwrap());

    //let tx = &pair.tx;
    //let rx = &pair.rx;

    let mut buf:Vec<i16> = Vec::new();
    //let mut num = 1;

    let min_speech_duration_seconds = 3.0;
    let max_speech_duration_seconds = 60.0;

    //let mut prev_sample:Option<Vec<i16>> = None;

    //let mut has_speech_time = 0.0;

    //let prev_size = ;

    // one second
    let mut prev_samples = AllocRingBuffer::<i16>::new(TARGET_SAMPLE_RATE as usize);



    thread::scope(|s| {
        s.spawn(move || {

            let mut prev_tag = SpeechTag::NoSpeech;

            let mut has_speech;

            let mut model = get_vad().unwrap();
            for samples in rx {
                if let Some(samples)=samples {
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
                    match prev_tag {
                    SpeechTag::NoSpeech => {
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
                    SpeechTag::HasSpeech => {
                        if seconds > max_speech_duration_seconds {
                            panic!("has speech > max_seconds {}, should treat as no speech and perhaps reset state but currently doesn't support reset state yet", seconds);
                            //eprintln!("override to to no speech because seconds > max_seconds {}", seconds);
                            //has_speech = false;
                        } else if seconds < min_speech_duration_seconds {
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
                            output_callback(&buf);
                            buf.clear();
                            prev_samples.clear();
                        }
                    }
                }

                prev_tag = SpeechTag::convert(has_speech);
            } else {
                eprintln!("Received end of stream signal");
                break;
            }

            };

            eprintln!("End of stream");

            if buf.len() > 0 {
                output_callback(&buf);
                buf.clear();
                //num += 1;
            }
            eprintln!("finished processing");
        });
        
        input_callback();
        //streaming_url(url,TARGET_SAMPLE_RATE,SAMPLE_SIZE,&tx).unwrap();
        
    });

    Ok(())
}




fn save_buf_to_file(buf: &Vec<i16>, file_name: &str) {
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

fn get_vad() -> Result<VoiceActivityDetector, Box<dyn std::error::Error>> {

    let model_path = get_silero_model()?;

    Ok(VoiceActivityDetector::build(TARGET_SAMPLE_RATE,SAMPLE_SIZE,&model_path))
}

pub fn stream_to_file(config: Config) -> Result<(), Box<dyn std::error::Error>>{
    //let url = "https://rthkradio2-live.akamaized.net/hls/live/2040078/radio2/master.m3u8";
    //let url = "https://www.am1430.net/wp-content/uploads/show/%E7%B9%BC%E7%BA%8C%E6%9C%89%E5%BF%83%E4%BA%BA/2023/2024-10-03.mp3";
    //println!("First argument: {}", first_argument);

    let url = config.url.as_str();

    let mut num = 1;
    let closure_annotated = |buf: &Vec<i16>| {
        let file_name = format!("tmp/predict.stream.speech.{}.wav", format!("{:0>3}",num));
        save_buf_to_file(&buf, &file_name);
        num += 1;
    };

    let (tx, rx) = bounded::<Option<Vec<i16>>>((TARGET_SAMPLE_RATE*60).try_into().unwrap());


    process_buffer_with_vad(&rx,
         || {
            streaming_url(url,TARGET_SAMPLE_RATE,SAMPLE_SIZE,&tx).unwrap();
        },
        closure_annotated)?;

        eprintln!("finished streaming to file");
    Ok(())
}

// also saves to db if database_name is provided in config
pub fn transcribe_url(config: Config,num_transcribe_threads: Option<usize>,model_download_url: &str) -> Result<(), Box<dyn std::error::Error>> {

    let rt = build_current_thread_runtime()?;

    let url: &str = config.url.as_str();
    let mut pool: Option<Pool<_>> = None;

    if let Some(database_config) = &config.database_config {
        let database_password = get_password(&database_config.database_password_key)?;
        //println!("My password is '{}'", database_password);

        pool = rt.block_on(async {
            //let path = Path::new(".").join("tmp").join(format!("{}.sqlite",database_name));
            // let pool2 = SqlitePool::connect_with(SqliteConnectOptions::new().filename(&path)
            //     .create_if_missing(true)).await.unwrap();
            // sqlx::query(r#"CREATE TABLE IF NOT EXISTS transcripts (
            //             id INTEGER PRIMARY KEY,
            //             timestamp datetime NOT NULL,
            //             content TEXT NOT NULL
            //     );"#
            // ).execute(&pool2).await?;
            let ssl_mode = match database_config.require_ssl {
                true => sqlx::postgres::PgSslMode::Require,
                _ => sqlx::postgres::PgSslMode::Prefer
            };
            let pool2 = PgPoolOptions::new().connect_with(PgConnectOptions::new()
                .ssl_mode(ssl_mode)
                .host(&database_config.database_host)
                .port(database_config.database_port.unwrap_or(5432))
                .database(database_config.database_name.as_str())
                .username(database_config.database_user.as_str())
                .password(database_password.as_str())
            ).await?;
            sqlx::query(r#"CREATE TABLE IF NOT EXISTS transcripts (
                id serial PRIMARY KEY,
                show_name varchar(255) NOT NULL,
                "timestamp" TIMESTAMP WITHOUT TIME ZONE NOT NULL,
                content TEXT NOT NULL
                );"#
                ).execute(&pool2).await?;
            sqlx::query(r#"create index if not exists transcript_show_name_idx ON transcripts (show_name);"#
                ).execute(&pool2).await?;

            Ok::<Option<Pool<_>>, Box<dyn std::error::Error>>(Some(pool2))
        })?;
    }


    // Load a context and model.
    let context_param = WhisperContextParameters::default();

    let download_url = model_download_url;

    let model_path = get_whisper_model(download_url)?;


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

        // Get the current timestamp using chrono
        let current_timestamp = Utc::now();

        let line = json!({"start_timestamp":data.start_timestamp,
            "end_timestamp":data.end_timestamp, "cur_ts": format!("{}",current_timestamp.format("%+")), "text":data.text});
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
        if let Some(pool) = &pool {
            rt.block_on(async {
                let sql = r#"INSERT INTO transcripts (show_name,"timestamp", content) VALUES ($1, $2, $3)"#;
                //eprint!("{}", sql);
                sqlx::query(
                    sql,
                )
                .bind(config.show_name.as_str())
                .bind(current_timestamp)
                .bind(db_save_text)
                .execute(pool).await?;
                Ok::<(), Box<dyn std::error::Error>>(())    
            }).unwrap();
        }

    });


    let mut state = ctx.create_state().expect("failed to create key");


    //let whisper_wrapper_ref = RefCell::new(whisper_wrapper);
    //let whisper_wrapper_ref2 = &whisper_wrapper;
    let closure_annotated = |buf: &Vec<i16>| {

        transcribe(&mut state, &params, &buf);

    };

    //let mut model = get_vad();

    if url == "microphone://default" {
        let mic_channel_pair = &*MIC_CHANNEL_PAIR;
        process_buffer_with_vad(&mic_channel_pair.rx,
            || {
                //loop {
                record_from_mic(&mic_channel_pair.tx,SAMPLE_SIZE).unwrap();
                //}
            },
            closure_annotated)?;
    }else if url == "audio_output://default" {
        return Err("audio_output://default not supported".into());
        // let mic_channel_pair = &*MIC_CHANNEL_PAIR;
        // process_buffer_with_vad(&mic_channel_pair.rx,
        //     || {
        //         //loop {
        //         record_computer_output(&mic_channel_pair.tx,SAMPLE_SIZE).unwrap();
        //         //}
        //     },
        //     closure_annotated)?;
    } else {

        let (tx, rx) = bounded::<Option<Vec<i16>>>((TARGET_SAMPLE_RATE*60).try_into().unwrap());

        process_buffer_with_vad(&rx,
            || {
                streaming_url(url,TARGET_SAMPLE_RATE,SAMPLE_SIZE,&tx).unwrap();
            },closure_annotated)?;

    }
    Ok(())
}