use std::env;
use std::process;

use whisper_rs_test::vad_processor::stream_to_file;
use whisper_rs_test::vad_processor::transcribe_url;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {


    // Collect command-line arguments, skipping the first one (program's name)
    let args: Vec<String> = env::args().collect();

    // Check if the first argument (after the program name) is provided
    if args.len() < 2 {
        eprintln!("Usage: {} <save_to_file|transcribe>", args[0]);
        process::exit(1);
    }

    // Get the first argument (the second element in the args vector)
    let operation = &args[1];
    


    match operation.as_str() {
        "save_to_file"=>{
            let url = "https://www.am1430.net/wp-content/uploads/show/%E7%B9%BC%E7%BA%8C%E6%9C%89%E5%BF%83%E4%BA%BA/2023/2024-10-03.mp3";
            stream_to_file(url).await?;
        },
        "transcribe"=>{
            let url = "https://rthkradio2-live.akamaized.net/hls/live/2040078/radio2/master.m3u8";
            log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
            whisper_rs::install_whisper_log_trampoline();
            transcribe_url(url).await?;
        },
        _=>{
            eprintln!("Usage: {} <save_to_file|transcribe>", args[0]);
            process::exit(1);
        }
    }

    
    Ok(())
}
