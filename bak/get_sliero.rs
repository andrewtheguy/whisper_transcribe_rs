use std::fs;

async fn get_silero() -> silero::Silero {

    let download_url = "https://github.com/snakers4/silero-vad/raw/refs/tags/v5.1/src/silero_vad/data/silero_vad.onnx";

    let model_local_directory = dirs::cache_dir().unwrap().join("whisper_transcribe_rs");
    fs::create_dir_all(&model_local_directory).unwrap();
    let file_name = get_filename_from_url(download_url).unwrap();
    let model_path = model_local_directory.join(file_name);
    if !model_path.exists() {
        eprintln!("Downloading model from {} to {}", download_url, model_path.to_str().unwrap());
        download_to_temp_and_move(download_url, model_path.to_str().unwrap()).await.unwrap();
    }

    let sample_rate = match TARGET_SAMPLE_RATE {
        8000 => utils::SampleRate::EightkHz,
        16000 => utils::SampleRate::SixteenkHz,
        _ => panic!("Unsupported sample rate. Expect 8 kHz or 16 kHz."),
    };

    let silero = silero::Silero::new(sample_rate, model_path).unwrap();
    silero
}