use std::{path};
use ggez::{error::GameResult, filesystem, graphics, Context};

pub struct AnimSettings {
  pub initial_offset_beats: u32,
  pub play_interval_beats: u32,
  pub length_ms: u32,
  pub beat_offset_ms: u32,
}

pub struct AnimAsset {
  frames: Vec<graphics::Image>,
  settings: AnimSettings,
}

impl AnimAsset {
  pub fn new<P: AsRef<path::Path>>(ctx: &mut Context, dir: P, settings: AnimSettings) -> GameResult<AnimAsset> {
    let mut frame_paths: Vec<path::PathBuf> = filesystem::read_dir(ctx, dir)?
      .into_iter().filter_map(|path| {
        match path.extension() {
          Some(ext) if ext == "png" => Some(path),
          _ => None
        }
      })
      .collect();
    frame_paths.sort();
    let frames = frame_paths.into_iter().map(|frame_path| {
      graphics::Image::new(ctx, frame_path).expect("Loading anim frame")
    }).collect();

    Ok(AnimAsset{ frames: frames, settings: settings })
  }
}

pub struct Animation {
}

impl Animation {
}