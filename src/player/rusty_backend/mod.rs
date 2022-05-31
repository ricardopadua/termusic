#![cfg_attr(test, deny(missing_docs))]

mod conversions;
mod sink;
mod stream;

pub mod buffer;
pub mod decoder;
pub mod dynamic_mixer;
pub mod queue;
pub mod source;

pub use conversions::Sample;
pub use cpal::{
    self, traits::DeviceTrait, Device, Devices, DevicesError, InputDevices, OutputDevices,
    SupportedStreamConfig,
};
pub use decoder::Symphonia;
pub use sink::Sink;
pub use source::Source;
pub use stream::{OutputStream, OutputStreamHandle, PlayError, StreamError};

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use super::GeneralP;
use crate::config::Termusic;
use anyhow::Result;

static VOLUME_STEP: u16 = 5;
static SEEK_STEP: f64 = 5.0;

pub struct Player {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Sink,
    total_duration: Option<Duration>,
    volume: u16,
    speed: f32,
    pub gapless: bool,
    current_item: Option<String>,
    next_item: Option<String>,
}

impl Player {
    pub fn new(config: &Termusic) -> Self {
        let (stream, handle) = OutputStream::try_default().unwrap();
        let gapless = config.gapless;
        let sink = Sink::try_new(&handle, gapless).unwrap();
        let volume = config.volume.try_into().unwrap();
        sink.set_volume(f32::from(volume) / 100.0);
        let speed = config.speed;
        sink.set_speed(speed);

        Self {
            _stream: stream,
            handle,
            sink,
            total_duration: None,
            volume,
            speed,
            gapless,
            current_item: None,
            next_item: None,
        }
    }
    pub fn play(&mut self, current_item: &Path, next_item: Option<&Path>) {
        if self.next_item.is_none() {
            self.stop();
        }
        self.current_item = Some(current_item.to_string_lossy().to_string());
        if self.current_item == self.next_item {
            // This is for gapless playback
            if let Some(next) = next_item {
                self.next_item = Some(next.to_string_lossy().to_string());
                if let Ok(file) = File::open(next) {
                    if let Ok(decoder) = Symphonia::new(file, self.gapless) {
                        // self.total_duration = decoder.total_duration();
                        self.sink.append(decoder);
                        // self.sink.set_speed(self.speed);
                    }
                }
            }

            return;
        }

        if let Ok(file) = File::open(current_item) {
            if let Ok(decoder) = Symphonia::new(file, self.gapless) {
                self.total_duration = decoder.total_duration();
                self.sink.append(decoder);
                self.sink.set_speed(self.speed);
            }
        }
    }
    pub fn stop(&mut self) {
        self.sink = Sink::try_new(&self.handle, self.gapless).unwrap();
        self.sink.set_volume(f32::from(self.volume) / 100.0);
    }
    pub fn elapsed(&self) -> Duration {
        self.sink.elapsed()
    }
    pub fn duration(&self) -> Option<f64> {
        self.total_duration
            .map(|duration| duration.as_secs_f64() - 0.29)
    }

    pub fn seek_fw(&mut self) {
        let new_pos = self.elapsed().as_secs_f64() + SEEK_STEP;
        if let Some(duration) = self.duration() {
            if new_pos < duration - SEEK_STEP {
                self.seek_to(Duration::from_secs_f64(new_pos));
            }
        }
    }
    pub fn seek_bw(&mut self) {
        let mut new_pos = self.elapsed().as_secs_f64() - SEEK_STEP;
        if new_pos < 0.0 {
            new_pos = 0.0;
        }

        self.seek_to(Duration::from_secs_f64(new_pos));
    }
    pub fn seek_to(&self, time: Duration) {
        self.sink.seek(time);
    }
    pub fn percentage(&self) -> f64 {
        self.duration().map_or(0.0, |duration| {
            let elapsed = self.elapsed();
            elapsed.as_secs_f64() / duration
        })
    }
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
        self.sink.set_speed(speed);
    }
}

impl GeneralP for Player {
    fn add_and_play(&mut self, current_track: &str, next_track: Option<&str>) {
        let p1 = Path::new(current_track);
        let p2 = next_track.map(Path::new);
        self.play(p1, p2);
    }

    fn volume(&self) -> i32 {
        self.volume.into()
    }

    fn volume_up(&mut self) {
        let volume = i32::from(self.volume) + i32::from(VOLUME_STEP);
        self.set_volume(volume);
    }

    fn volume_down(&mut self) {
        let volume = i32::from(self.volume) - i32::from(VOLUME_STEP);
        self.set_volume(volume);
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn set_volume(&mut self, mut volume: i32) {
        if volume > 100 {
            volume = 100;
        } else if volume < 0 {
            volume = 0;
        }
        self.volume = volume as u16;
        self.sink.set_volume(f32::from(self.volume) / 100.0);
    }

    fn pause(&mut self) {
        self.sink.pause();
    }

    fn resume(&mut self) {
        self.sink.play();
    }

    fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    fn seek(&mut self, secs: i64) -> Result<()> {
        if secs.is_positive() {
            self.seek_fw();
            return Ok(());
        }

        self.seek_bw();
        Ok(())
    }

    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation
    )]
    fn get_progress(&mut self) -> Result<(f64, i64, i64)> {
        let position = self.elapsed().as_secs() as i64;
        let duration = self.duration().unwrap_or(99.0) as i64;
        let mut percent = self.percentage() * 100.0;
        if percent > 100.0 {
            percent = 100.0;
        }
        Ok((percent, position, duration))
    }

    fn speed_up(&mut self) {
        let mut speed = self.speed + 0.1;
        if speed > 3.0 {
            speed = 3.0;
        }
        self.set_speed(speed);
    }

    fn speed_down(&mut self) {
        let mut speed = self.speed - 0.1;
        if speed < 0.1 {
            speed = 0.1;
        }
        self.set_speed(speed);
    }

    fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
        self.set_speed(speed);
    }

    fn speed(&self) -> f32 {
        self.speed
    }
    fn stop(&mut self) {
        self.stop();
    }
}
