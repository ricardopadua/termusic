use parking_lot::Mutex;
use std::sync::Arc;
use termusic_stream::StreamDownload;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::default().add_directive("stream_download=trace".parse().unwrap()),
        )
        .with_line_number(true)
        .with_file(true)
        .init();
    let (_stream, handle) = rodio::OutputStream::try_default().unwrap();
    let sink = rodio::Sink::try_new(&handle).unwrap();

    let reader = StreamDownload::new_http(
        "https://uk1.internet-radio.com/proxy/pinknoise?mp=/stream"
            .parse()
            .unwrap(),
        true,
        Arc::new(Mutex::new(String::new())),
        Arc::new(Mutex::new(0)),
    )
    .unwrap();

    sink.append(rodio::Decoder::new(reader).unwrap());

    sink.sleep_until_end();
}
