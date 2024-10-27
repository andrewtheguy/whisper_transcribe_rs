use crossbeam::channel::{bounded, unbounded, Receiver};
use hound::{self};
use log::{debug, trace};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
//use sqlx::sqlite::{SqliteConnectOptions};
use sqlx::{pool, Pool, Postgres};
use ringbuffer::{AllocRingBuffer, RingBuffer};
use tokio::runtime::Runtime;

use crate::config::DatabaseConfig;
use crate::download_utils::{get_whisper_model, get_silero_model};
use crate::key_ring_utils::get_password;
use crate::runtime_utils::{get_runtime};
use crate::streaming::Segment;
use crate::web::TranscribeWebServer;
use crate::{config::Config, streaming::streaming_url, vad::VoiceActivityDetector};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use std::path::PathBuf;
use std::thread;
use serde_json::json;

use zhconv::{zhconv, Variant};

use std::thread::available_parallelism;

use chrono::{TimeZone, Utc};


use crate::record_audio::record_from_mic;


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

pub const TARGET_SAMPLE_RATE: i64 = 16000;
pub const SAMPLE_SIZE: usize = 1024;


/*
The VAD predicts speech in a chunk of Linear Pulse Code Modulation (LPCM) encoded audio samples. These may be 8 or 16 bit integers or 32 bit floats.

The model is trained using chunk sizes of 256, 512, and 768 samples for an 8000 hz sample rate. It is trained using chunk sizes of 512, 768, 1024 samples for a 16,000 hz sample rate.
*/

fn process_with_vad<E,F>(rx: &Receiver<Option<Segment>>, input_callback: E, mut output_callback: F) -> Result<(), Box<dyn std::error::Error>>
where
    E: FnOnce() + std::marker::Send,
    F: FnMut(Option<i64>,&Vec<i16>) + std::marker::Send,
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

            let mut has_speech_begin_timestamp: Option<i64> = None;

            let mut model = get_vad().unwrap();
            for segment in rx {
                if let Some(segment)=segment {
                    let samples = segment.samples;
                    trace!("Received sample size: {}", samples.len());
                    //assert!(samples.len() as i32 == target_sample_rate); //make sure it is one second
                    //let sample2 = samples.clone();
                    //silero.reset();
                    //let mut rng = rand::thread_rng();
                    //let probability: f64 = rng.gen();
                    let probability = model.predict(samples.clone());
                    //let len_after_samples: i32 = (buf.len() + samples.len()).try_into().unwrap();
                    trace!("buf.len() {}", buf.len());
                    let seconds = buf.len() as f32 / TARGET_SAMPLE_RATE as f32;
                    //trace!("len_after_samples / target_sample_rate {}",seconds);

                    if probability > 0.5 {
                        trace!("Chunk is speech: {}", probability);
                        has_speech = true;
                    } else {
                        has_speech = false;
                    }

                    assert!(prev_samples.len()<=TARGET_SAMPLE_RATE as usize);
                    match prev_tag {
                    SpeechTag::NoSpeech => {
                        if has_speech {
                            trace!("Transitioning from no speech to speech");
                            // save the timestamp
                            has_speech_begin_timestamp = Some(segment.timestamp_millis);
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
                            // maybe reset silero state if no speech for too long
                            trace!("Still No Speech");
                            prev_samples.extend(samples.iter().cloned());
                        }
                    },
                    SpeechTag::HasSpeech => {
                        if seconds > max_speech_duration_seconds {
                            //maybe reset silero state
                            debug!("override to no speech because seconds > max_seconds {}", seconds);
                            has_speech = false;
                        } else if seconds < min_speech_duration_seconds {
                            debug!("override to Continue to has speech because seconds < min_seconds {}", seconds);
                            has_speech = true;
                        }
                        if has_speech {
                            trace!("Continue to has speech");
                            // continue to extend the buffer
                            buf.extend(&samples);
                        } else {
                            trace!("Transitioning from speech to no speech");
                            buf.extend(&samples);
                            //save the buffer if not empty
                            output_callback(has_speech_begin_timestamp,&buf);
                            has_speech_begin_timestamp = None;
                            buf.clear();
                            prev_samples.clear();
                        }
                    }
                }

                prev_tag = SpeechTag::convert(has_speech);
            } else {
                debug!("Received end of stream signal");
                break;
            }

            };

            debug!("End of stream");

            if buf.len() > 0 {
                output_callback(has_speech_begin_timestamp,&buf);
                has_speech_begin_timestamp = None;
                buf.clear();
                //num += 1;
            }
            debug!("finished processing");
        });
        
        // needed this for the thread not to block
        input_callback();
        //streaming_url(url,TARGET_SAMPLE_RATE,SAMPLE_SIZE,&tx).unwrap();
        
    });

    Ok(())
}




pub fn save_buf_to_file(buf: &Vec<i16>, file_name: &PathBuf) {
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


    // Create an audio buffer to hold the audio samples.
    let mut audio = vec![0.0f32; samples.len().try_into().unwrap()];

    whisper_rs::convert_integer_to_float_audio(&samples, &mut audio).expect("Conversion error");

    // Run the model.
    state.full(params.clone(), &audio[..]).expect("failed to run model");

    /*

    let language = config.language.clone();

    use this if wait until it finishes
    let rt = get_runtime();

	// fetch the results
	let num_segments = state
		.full_n_segments()
		.expect("failed to get number of segments");
	for i in 0..num_segments {
		let text = state
			.full_get_segment_text(i)
			.expect("failed to get segment");
		let start_timestamp = state
			.full_get_segment_t0(i)
			.expect("failed to get segment start timestamp");
		let end_timestamp = state
			.full_get_segment_t1(i)
			.expect("failed to get segment end timestamp");


        // Get the current timestamp using chrono
        let current_timestamp = Utc::now();

        let line = json!({"start_timestamp":start_timestamp,
            "end_timestamp":end_timestamp, "cur_ts": format!("{}",current_timestamp.to_rfc3339()), "text":text});
        println!("{}", line);

        // only convert to traditional chinese when saving to db
        // output original in jsonl
        let db_save_text = match language.as_str() {
            "zh" | "yue" => {
                zhconv(&text, Variant::ZhHant)
            },
            _ => {
                text
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

	}
     */
}

fn get_vad() -> Result<VoiceActivityDetector, Box<dyn std::error::Error>> {

    let model_path = get_silero_model()?;

    Ok(VoiceActivityDetector::build(TARGET_SAMPLE_RATE,SAMPLE_SIZE,&model_path))
}

pub fn stream_to_file(config: Config) -> Result<(), Box<dyn std::error::Error>>{

    let folder = std::path::Path::new("tmp").join(&config.show_name);

    match std::fs::remove_dir_all(&folder) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }.expect("failed to remove folder to save files");

    std::fs::create_dir_all(&folder).expect("failed to create folder to save files");

    let mut num = 1;
    let closure_annotated = |_has_speech_begin_timestamp,buf: &Vec<i16>| {
        let file_name = folder.join(format!("predict.stream.speech.{}.wav", format!("{:0>3}",num)));
        save_buf_to_file(&buf, &file_name);
        num += 1;
    };

    match &config.source {
        crate::config::Source::Url => {

            let (tx, rx) = bounded::<Option<Segment>>((TARGET_SAMPLE_RATE*60).try_into().unwrap());

            let url = config.url.as_ref().expect("url is required when source is url");

            process_with_vad(&rx,
                || {
                   streaming_url(url,TARGET_SAMPLE_RATE,SAMPLE_SIZE,&tx).unwrap();
               },
               closure_annotated)?;

               debug!("finished streaming to file");
        },
        crate::config::Source::Microphone => {
            let (tx, rx) = unbounded::<Option<Segment>>().try_into().unwrap();
            process_with_vad(&rx,
                || {
                    record_from_mic(&tx,SAMPLE_SIZE).unwrap();
                },
                closure_annotated)?;
        },
        crate::config::Source::Web => {
            let (tx, rx) = unbounded::<Option<Segment>>().try_into().unwrap();
            let rt = get_runtime();

            let pool = if let Some(database_config) = &config.database_config {
                init_db_from_config(rt,database_config)?
            } else {
                panic!("database config is required when source is web for pulling data for web");
            };

            process_with_vad(&rx,
                || {
                    rt.block_on(async {
                        TranscribeWebServer::new(5002,tx.clone(),pool).start_webserver().await
                    });
                },
                closure_annotated)?;
        },
    }

    Ok(())
}

fn init_db_from_config(rt: &Runtime,database_config: &DatabaseConfig) -> Result<Pool<sqlx::Postgres>, Box<dyn std::error::Error>> {
    let database_password = get_password(&database_config.database_password_key)?;
    //println!("My password is '{}'", database_password);

    rt.block_on(async {
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
            id bigserial PRIMARY KEY,
            show_name varchar(255) NOT NULL,
            "timestamp" TIMESTAMP WITHOUT TIME ZONE NOT NULL,
            content TEXT NOT NULL
            );"#
            ).execute(&pool2).await?;
        sqlx::query(r#"create index if not exists transcript_show_name_idx ON transcripts (show_name);"#
            ).execute(&pool2).await?;

        Ok::<Pool<Postgres>, Box<dyn std::error::Error>>(pool2)
    })
}

// also saves to db if database_name is provided in config
pub fn transcribe_url(config: Config,num_transcribe_threads: Option<usize>,model_download_url: &str) -> Result<(), Box<dyn std::error::Error>> {

    let rt = get_runtime();
    //eprintln!("transcribe_url");

    let source = &config.source;
    let mut pool: Option<Pool<_>> = None;

    if let Some(database_config) = &config.database_config {
        pool = Some(init_db_from_config(rt,database_config)?);
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


    let mut state = ctx.create_state().expect("failed to create key");


    //let whisper_wrapper_ref = RefCell::new(whisper_wrapper);
    //let whisper_wrapper_ref2 = &whisper_wrapper;
    let closure_annotated = |timestamp_millis,buf: &Vec<i16>| {

        let mut params2 = params.clone();
        let language2 = language.clone();
        let show_name2 = config.show_name.clone();
        // safe to clone https://github.com/launchbadge/sqlx/discussions/917
        let pool2 = pool.clone();
        let rt2= rt.clone();
        //let buf2 = buf.clone();
        params2.set_segment_callback_safe( move |data: whisper_rs::SegmentCallbackData| {
            //let buf3 = buf2.clone();


            let calculated_start_timestamp_obj = if let Some(timestamp_millis) = timestamp_millis{
              Some(Utc.timestamp_millis_opt(timestamp_millis+data.start_timestamp).unwrap())
            }else{
                None
            };

            let calculated_start_timestamp = match calculated_start_timestamp_obj {
                Some(ts) => Some(ts.to_rfc3339()),
                None => None
            };

            //let calculated_end_timestamp = Utc.timestamp_millis_opt(timestamp_millis+data.end_timestamp);

            let line = json!({"start_timestamp":data.start_timestamp,
                "end_timestamp":data.end_timestamp, "cur_ts": calculated_start_timestamp, "text":data.text});
            println!("{}", line);

            // only convert to traditional chinese when saving to db
            // output original in jsonl
            let db_save_text = match language2.as_str() {
                "zh" | "yue" => {
                    zhconv(&data.text, Variant::ZhHant)
                },
                _ => {
                    data.text
                }
            };
            if let Some(pool) = &pool2 {
                // need to fallback to current timestamp if calculated_start_timestamp is None
                let current_timestamp_db_save = match calculated_start_timestamp_obj {
                    Some(ts) => ts,
                    None => Utc::now()
                };
                rt2.block_on(async {
                    let sql = r#"INSERT INTO transcripts (show_name,"timestamp", content) VALUES ($1, $2, $3)"#;
                    //eprint!("{}", sql);
                    sqlx::query(
                        sql,
                    )
                    .bind(show_name2.as_str())
                    .bind(current_timestamp_db_save)
                    .bind(db_save_text)
                    .execute(pool).await?;
                    Ok::<(), Box<dyn std::error::Error>>(())    
                }).unwrap();
            }

        });

        transcribe(&mut state, &params2, &buf);

    };

    match source {
        crate::config::Source::Url => {
            let url = config.url.as_ref().expect("url is required when source is url");
            let (tx, rx) = bounded::<Option<Segment>>((TARGET_SAMPLE_RATE*60).try_into().unwrap());
            process_with_vad(&rx,
                || {
                    streaming_url(url,TARGET_SAMPLE_RATE,SAMPLE_SIZE,&tx).unwrap();
                },
                closure_annotated)?;
        },
        crate::config::Source::Microphone => {
            let (tx, rx) = unbounded::<Option<Segment>>().try_into().unwrap();
            process_with_vad(&rx,
                || {
                    record_from_mic(&tx,SAMPLE_SIZE).unwrap();
                },
                closure_annotated)?;
        },
        crate::config::Source::Web => {
            let (tx, rx) = unbounded::<Option<Segment>>().try_into().unwrap();
            let rt = get_runtime();

            let pool2 = pool.clone().expect("database pool is required when source is web for pulling data for web");

            process_with_vad(&rx,
                || {
                    let pool2 = pool2.clone();
                    rt.block_on(async {
                        TranscribeWebServer::new(5002,tx.clone(),pool2).start_webserver().await
                    });
                },
                closure_annotated)?;

        },
    }

    Ok(())
}