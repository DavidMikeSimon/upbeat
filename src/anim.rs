use std::{path, rc::Rc};
use ggez::{error::GameResult, filesystem, graphics, Context};

#[derive(Default)]
pub struct AnimSettings {
  pub initial_offset_beats: u32,
  pub play_interval_beats: u32,
  pub length_ms: u32,
  pub hide_between_plays: bool,
}

pub struct AnimAsset {
  frames: Vec<graphics::Image>,
  settings: AnimSettings,
}

impl AnimAsset {
  pub fn new<P: AsRef<path::Path>>(ctx: &mut Context, src: P, settings: AnimSettings) -> GameResult<AnimAsset> {
    let mut frame_paths: Vec<path::PathBuf> = match src.as_ref().extension() {
      Some(ext) if ext == "png" => {
        vec!(src.as_ref().to_path_buf())
      },
      _ => {
        filesystem::read_dir(ctx, src)?
          .into_iter().filter_map(|path| {
            match path.extension() {
              Some(ext) if ext == "png" => Some(path),
              _ => None
            }
          })
          .collect()
      }
    };

    frame_paths.sort();
    let frames = frame_paths.into_iter().map(|frame_path| {
      graphics::Image::new(ctx, frame_path).expect("Loading anim frame")
    }).collect();

    Ok(AnimAsset{ frames: frames, settings: settings })
  }
}

pub struct Animation {
  asset: Rc<AnimAsset>,
}

impl Animation {
  pub fn new(asset: Rc<AnimAsset>) -> Animation {
    Animation { asset: asset }
  }

  pub fn get_frame(&self, time: u32, ms_per_beat: f32) -> Option<&graphics::Image> {
    let default_frame = match self.asset.settings.hide_between_plays {
      false => Some(&self.asset.frames[0]),
      true => None
    };

    if self.asset.frames.len() == 1 { 
      return default_frame;
    }

    let initial_offset: u32 = (self.asset.settings.initial_offset_beats as f32 * ms_per_beat) as u32;
    if time <= initial_offset {
      return default_frame;
    }
    let play_interval_ms: u32 = (self.asset.settings.play_interval_beats as f32 * ms_per_beat) as u32;
    let time = (time - initial_offset) % play_interval_ms;
    if time >= self.asset.settings.length_ms {
      return default_frame;
    }

    let f = ((time * self.asset.frames.len() as u32) / self.asset.settings.length_ms) as usize;
    return Some(&self.asset.frames[f]);
  }
}