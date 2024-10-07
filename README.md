sample setup to transcribe with whisper.cpp's rust binding
it will convert the file with ffmpeg to the waveform in memory first
need to install ffmpeg separately
create a config toml file
```
cargo run -- config.toml 2> >(rotatelogs -n 5 ./tmp/output.log 1M >&2)
```
