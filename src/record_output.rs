
use coreaudio::audio_unit::{AudioUnit, OutputUnit};
use coreaudio::audio_unit::playback::{AUDIO_UNIT_TYPE_OUTPUT, kAudioUnitSubType_RemoteIO};
use coreaudio::audio_unit::AudioUnitType;
use coreaudio::audio_toolbox::{AudioBufferList, AudioStreamBasicDescription};
use coreaudio::sys::{kAudioUnitScope_Global, kAudioUnitScope_Output};


fn main() {
    // Initialize the Audio Unit for RemoteIO
    let audio_unit = AudioUnit::new(AUDIO_UNIT_TYPE_OUTPUT).expect("Failed to create Audio Unit");

    // Enable input on the Audio Unit
    audio_unit.set_property(
        coreaudio::audio_unit::properties::EnableInput,
        &1,
    ).expect("Failed to enable input");

    // Set up the audio format (example: 44.1 kHz, 2 channels, float)
    let audio_format = AudioStreamBasicDescription {
        mSampleRate: 44100.0,
        mFormatID: coreaudio::audio_format::kAudioFormatLinearPCM,
        mFormatFlags: coreaudio::audio_format::kAudioFormatFlagIsFloat | coreaudio::audio_format::kAudioFormatFlagIsPacked,
        mBytesPerPacket: 8,
        mFramesPerPacket: 1,
        mBytesPerFrame: 8,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 32,
        mReserved: 0,
    };

    audio_unit.set_property(
        coreaudio::audio_unit::properties::StreamFormat,
        &audio_format,
    ).expect("Failed to set stream format");

    // Set the input callback to handle incoming audio data
    audio_unit.set_input_callback(Some(Box::new(move |data: &AudioBufferList| {
        // Process audio data here
        println!("Received audio data with {} buffers", data.mNumberBuffers);
        // Here you can save data to a file or perform further processing
    }))).expect("Failed to set input callback");

    // Initialize and start the audio unit
    audio_unit.initialize().expect("Failed to initialize Audio Unit");
    audio_unit.start().expect("Failed to start Audio Unit");

    println!("Recording... Press Enter to stop.");
    let _ = std::io::stdin().read_line(&mut String::new());

    // Stop and clean up
    audio_unit.stop().expect("Failed to stop Audio Unit");
    audio_unit.dispose().expect("Failed to dispose Audio Unit");
}