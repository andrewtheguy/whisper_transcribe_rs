use std::fs;
use std::process;

// Trait for extending std::path::Path
use path_slash::PathExt as _;

use whisper_transcribe_rs::config::Operation;
use whisper_transcribe_rs::key_ring_utils;
use whisper_transcribe_rs::vad_processor::stream_to_file;
use whisper_transcribe_rs::vad_processor::transcribe_url;
use whisper_transcribe_rs::config::Config;
use whisper_transcribe_rs::utils::get_config_dir;
use std::io::Write;
//use whisper_transcribe_rs::log_builder::MyLoggerBuilder;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    //name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "config", help = "config to operate on")]
    config_file: PathBuf,

    /// Turn debugging information on
    // #[arg(short, long, action = clap::ArgAction::Count)]
    // debug: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {

    #[command(about = "process url from config file to transcribe or save to file")]
    ProcessUrl {

        #[arg(short, long)]
        model: Option<String>,

        #[arg(short, long)]
        num_transcribe_threads: Option<usize>,
    },
    #[command(about = "set database password from config file")]
    SetDbPassword
}

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let cli = Cli::parse();
    
    let config: Config = toml::from_str(fs::read_to_string(cli.config_file)?.as_str()).unwrap();
    let subcommand = cli.command;
    match subcommand {
        Commands::ProcessUrl{ model, num_transcribe_threads} => {

            let default_download_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/4496f29dabb6f37d8e6c45c3ec89ccbe66a832ea/ggml-large-v3-turbo.bin?download=true";

            let mut model_download_url = default_download_url;
            if let Some(model) = model {
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
        
            let config_folder = get_config_dir()?;
            eprintln!("config_folder: {}", config_folder.to_str().unwrap());

            let log_dir = config_folder.join("logs");
            fs::create_dir_all(&log_dir)?;

            let log_path = log_dir.join(format!("{}.log",config.show_name));
            let log_path_my_app = log_dir.join(format!("{}_streaming.log",config.show_name));

            let template = include_str!("log4rs.yaml");

            // Replace the placeholder with the actual log path
            let config_str = template.replace("{{log_path}}", &log_path.to_slash().unwrap())
                                             .replace("{{log_path_my_app}}", &log_path_my_app.to_slash().unwrap());


            let config_log = serde_yaml::from_str(config_str.as_str()).unwrap();
            log4rs::init_raw_config(config_log).unwrap();
        
            //error!("test error");

            //let operation = config.operation;
        
            match config.operation {
                Operation::SaveToFile=>{
                    //let url = "https://www.am1430.net/wp-content/uploads/show/%E7%B9%BC%E7%BA%8C%E6%9C%89%E5%BF%83%E4%BA%BA/2023/2024-10-03.mp3";
                    stream_to_file(config)?;
                },
                Operation::Transcribe=>{
                    whisper_rs::install_whisper_log_trampoline();
                    transcribe_url(config,num_transcribe_threads,model_download_url)?;
                },
                _=>{
                    eprintln!("unknown operation: {:?}", config.operation);
                    process::exit(1);
                }
            }
        
        },
        Commands::SetDbPassword => {
            let database_password_key = config.database_config.unwrap().database_password_key.clone();
            print!("Type the password for database_password_key {}: ", database_password_key);
            std::io::stdout().flush().unwrap();
            let mut buf: String = String::new();
            std::io::stdin().read_line(&mut buf)?;
            let password = buf.trim();
            //let password = entry.get_password()?;
            println!("My password is '{}'", password);
            key_ring_utils::set_password(&database_password_key, password)?;
            //return Ok(());
        },
    };
    
    Ok(())
}
