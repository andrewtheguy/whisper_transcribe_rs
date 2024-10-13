Sample setup to transcribe with whisper.cpp's rust binding
it will convert the url stream with ffmpeg to the waveform in memory first, then output to jsonl and at the same time save to database
need to install ffmpeg separately

- create a config toml file
- then run
```
cargo run -- config.toml 2> >(rotatelogs -n 5 ./tmp/output.log 1M >&2)
```

set postgres password:
```
cargo run --example set_pg_key_ring_password path_to_config.toml
```

windows:
```
cargo run -- config.toml 2> NUL
```
- see config*.toml for config examples

- still need improvement on silero vad to include clips before and after speech/no speech transitions

- need to convert eprintln! to log!

build linux arm binary:

```
DOCKER_BUILDKIT=1 docker build --platform linux/arm64 -o target/linux_arm64 .
```

