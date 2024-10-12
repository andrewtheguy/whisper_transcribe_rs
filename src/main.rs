use std::env;
use std::fs;
use std::process;
use clap::Arg;
use whisper_transcribe_rs::vad_processor::stream_to_file;
use whisper_transcribe_rs::vad_processor::transcribe_url;
use whisper_transcribe_rs::config::Config;

use std::path::PathBuf;

use log4rs;
use serde_yaml;

use clap::{arg, command, value_parser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = command!() // requires `cargo` feature
        .arg(arg!([config] "config to operate on").required(true).value_parser(value_parser!(PathBuf)))
        .arg(
            arg!(
                -m --model <OPTION> "model name (turbo, distil_small_en, ggml-small-q5_1)"
            ).value_parser(value_parser!(String))
        )
        .arg(
            Arg::new("num-transcribe-threads")
            .long("num-transcribe-threads")
            .required(false)
            .help("number of threads for transcribe, default 4 or maximum number of cpus if less available").value_parser(value_parser!(usize))
        )
        // .arg(arg!(
        //     -d --debug ... "Turn debugging information on"
        // ))
        // .subcommand(
        //     Command::new("test")
        //         .about("does testing things")
        //         .arg(arg!(-l --list "lists test values").action(ArgAction::SetTrue)),
        // )
        .get_matches();

        
    if let Some(config_file) = matches.get_one::<PathBuf>("config"){

        let config: Config = toml::from_str(fs::read_to_string(config_file)?.as_str()).unwrap();

        let default_download_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/4496f29dabb6f37d8e6c45c3ec89ccbe66a832ea/ggml-large-v3-turbo.bin?download=true";

        let mut model_download_url = default_download_url;
        if let Some(model) = matches.get_one::<String>("model") {
            match model.as_str() {
                "turbo"=>{
                    model_download_url = default_download_url;
                },
                "base_en" =>{
                    model_download_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin?download=true";
                },
                "distil_small_en" =>{
                    model_download_url = "https://huggingface.co/distil-whisper/distil-small.en/resolve/main/ggml-distil-small.en.bin?download=true";
                },
                "ggml-small-q5_1" =>{
                    model_download_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin?download=true";
                },
                _=>{
                    eprintln!("unknown model: {}", model);
                    process::exit(1);
                }
            }
        }
        
        let num_transcribe_threads = matches.get_one::<usize>("num-transcribe-threads");

        let operation = config.operation.as_str();

        match operation {
            "save_to_file"=>{
                //let url = "https://www.am1430.net/wp-content/uploads/show/%E7%B9%BC%E7%BA%8C%E6%9C%89%E5%BF%83%E4%BA%BA/2023/2024-10-03.mp3";
                stream_to_file(config)?;
            },
            "transcribe"=>{
                //let url = "https://rthkradio2-live.akamaized.net/hls/live/2040078/radio2/master.m3u8";
                let config_str = include_str!("log4rs.yaml");
                let config_log = serde_yaml::from_str(config_str).unwrap();
                log4rs::init_raw_config(config_log).unwrap();
                //log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
                whisper_rs::install_whisper_log_trampoline();
                transcribe_url(config,num_transcribe_threads.copied(),model_download_url)?;
            },
            _=>{
                eprintln!("unknown operation: {}", operation);
                process::exit(1);
            }
        }
    }else{
        eprintln!("config file not found");
        process::exit(1);
    }
    


    
    Ok(())
}
