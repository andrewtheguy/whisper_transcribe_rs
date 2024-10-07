sample setup to transcribe with whisper.cpp's rust binding
it will convert the file with ffmpeg to the waveform in memory first
need to install ffmpeg separately
```
cargo run -- transcribe 2> >(rotatelogs -n 5 ./tmp/output.log 1M >&2)
```

## test saving to wave file for debugging

```
cargo run --bin vad_streaming_file
```