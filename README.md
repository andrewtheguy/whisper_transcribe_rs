Sample setup to transcribe with whisper.cpp's rust binding
it will convert the url stream with ffmpeg to the waveform in memory first, then output to jsonl and at the same time save to database
need to install ffmpeg separately

- create a config toml file
- then run
```
cargo run -- --config-file config.toml process-url 2> >(rotatelogs -n 5 ./tmp/output.log 1M >&2)
```

set postgres password:
```
cargo run -- --config-file config_knx.toml set-db-password
```

windows:
```
cargo run -- config.toml 2> NUL
```
- see config*.toml for config examples

TODO:

- still need improvement on silero vad to include clips before and after speech/no speech transitions

- need to convert eprintln! to log!

- better timestamp detection

- find out why livestream sometimes dies without errors even if I have added an infinite loop on it

- should perhaps reset state if it continues to have speech or no speech for too long

build linux arm binary:

```
DOCKER_BUILDKIT=1 docker build --platform linux/arm64 -o target/linux_arm64 .
```

