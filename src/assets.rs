use std::{rc::Rc};

use ggez::{graphics, Context};
use nalgebra::{Point2};

use crate::anim;

pub struct Assets {
  pub font: graphics::Font,

  pub char1: graphics::Image,
  pub char2: graphics::Image,
  pub monster: graphics::Image,

  pub sky_anim: Rc<anim::AnimAsset>,
  pub grass_anim: Rc<anim::AnimAsset>,
  pub rocks_anim: Rc<anim::AnimAsset>,
  pub dirt_anim: Rc<anim::AnimAsset>,

  pub left_bush_anim: Rc<anim::AnimAsset>,
  pub right_bush_anim: Rc<anim::AnimAsset>,
  pub left_tree_anim: Rc<anim::AnimAsset>,
  pub right_tree_anim: Rc<anim::AnimAsset>,
  pub wind_anim: Rc<anim::AnimAsset>,

  pub after_attack_effect: graphics::Mesh,

  pub button_width: f32,
  pub button_margin: f32,
  pub cursor: graphics::Mesh,
  pub button: graphics::Mesh,

  pub music_bar_height: f32,
  pub music_bar: graphics::Mesh,

  pub now_line_width: f32,
  pub now_line_x_offset: f32,
  pub now_line: graphics::Mesh,

  pub measure_line: graphics::Mesh,
  pub measure_action_indicator: graphics::Mesh,

  pub arrow_width: f32,
  pub up_arrow: graphics::Mesh,
  pub down_arrow: graphics::Mesh,
}

impl Assets {
  pub fn new(ctx: &mut Context) -> Assets {
    let window = graphics::screen_coordinates(ctx);

    let font = graphics::Font::new(ctx, "/fonts/Catamaran/Catamaran-Regular.ttf").unwrap();

    let char1 = graphics::Image::new(ctx, "/images/battle_scene/perry blue.png").unwrap();
    let char2 = graphics::Image::new(ctx, "/images/char2.png").unwrap();
    let monster = graphics::Image::new(ctx, "/images/monster.png").unwrap();

    let sky_anim = anim::AnimAsset::new(ctx, "/images/battle_scene/sky.png", anim::AnimSettings::default()).unwrap();
    let grass_anim = anim::AnimAsset::new(ctx, "/images/battle_scene/grass.png", anim::AnimSettings::default()).unwrap();
    let rocks_anim = anim::AnimAsset::new(ctx, "/images/battle_scene/rocks.png", anim::AnimSettings::default()).unwrap();
    let dirt_anim = anim::AnimAsset::new(ctx, "/images/battle_scene/dirt.png", anim::AnimSettings::default()).unwrap();

    let left_bush_anim = anim::AnimAsset::new(ctx, "/images/battle_scene/left bush", anim::AnimSettings{
      play_interval_beats: 12,
      length_ms: 900,
      ..Default::default()
    }).unwrap();

    let right_bush_anim = anim::AnimAsset::new(ctx, "/images/battle_scene/right bush", anim::AnimSettings{
      play_interval_beats: 12,
      length_ms: 900,
      ..Default::default()
    }).unwrap();

    let left_tree_anim = anim::AnimAsset::new(ctx, "/images/battle_scene/left tree", anim::AnimSettings{
      initial_offset_beats: 3,
      play_interval_beats: 12,
      length_ms: 300,
      ..Default::default()
    }).unwrap();

    let right_tree_anim = anim::AnimAsset::new(ctx, "/images/battle_scene/right tree", anim::AnimSettings{
      initial_offset_beats: 3,
      play_interval_beats: 12,
      length_ms: 300,
      ..Default::default()
    }).unwrap();

    let wind_anim = anim::AnimAsset::new(ctx, "/images/battle_scene/wind", anim::AnimSettings{
      play_interval_beats: 24,
      length_ms: 1000,
      hide_between_plays: true,
      ..Default::default()
    }).unwrap();

    let after_attack_effect = graphics::Mesh::new_circle(
      ctx,
      graphics::DrawMode::fill(),
      Point2::new(0.0, 0.0),
      50.0,
      0.1,
      graphics::Color::from_rgb(255, 255, 255)
    ).unwrap();

    let button_width = 60.0;
    let button_margin = 5.0;

    let cursor = graphics::Mesh::new_rectangle(
      ctx,
      graphics::DrawMode::stroke(5.0),
      graphics::Rect::new(0.0, 0.0, button_width, button_width),
      graphics::Color::from_rgb(210, 250, 180)
    ).unwrap();

    let button = graphics::Mesh::new_rectangle(
      ctx,
      graphics::DrawMode::fill(),
      graphics::Rect::new(0.0, 0.0, button_width, button_width),
      graphics::Color::from_rgba(210, 210, 210, 128)
    ).unwrap();

    let music_bar_height = 200.0;

    let music_bar = graphics::Mesh::new_rectangle(
      ctx,
      graphics::DrawMode::fill(),
      graphics::Rect::new(0.0, 0.0, window.w, music_bar_height),
      graphics::Color::from_rgba(210, 210, 210, 128)
    ).unwrap();

    let now_line_width = 2.0;
    let now_line_x_offset = 250.0;

    let now_line = graphics::Mesh::new_line(
      ctx,
      &[
        Point2::new(0.0, 0.0),
        Point2::new(0.0, music_bar_height)
      ],
      now_line_width,
      graphics::BLACK
    ).unwrap();

    let measure_line_width = 2.0;
    let measure_line = graphics::Mesh::new_line(
      ctx,
      &[
        Point2::new(0.0, 0.0),
        Point2::new(0.0, music_bar_height)
      ],
      measure_line_width,
      graphics::Color::from_rgb(64, 64, 64)
    ).unwrap();

    let measure_action_indicator = graphics::Mesh::new_circle(
      ctx,
      graphics::DrawMode::fill(),
      Point2::new(0.0, 0.0),
      10.0,
      0.1,
      graphics::Color::from_rgb(255, 255, 255)
    ).unwrap();

    let arrow_width = 20.0;
    let arrow_height = 10.0;

    let up_arrow = graphics::Mesh::new_polygon(
      ctx,
      graphics::DrawMode::fill(),
      &[
        Point2::new(0.0, -arrow_height/2.0),
        Point2::new(arrow_width/2.0, arrow_height/2.0),
        Point2::new(-arrow_width/2.0, arrow_height/2.0),
      ],
      graphics::Color::from_rgb(0, 192, 32)
    ).unwrap();

    let down_arrow = graphics::Mesh::new_polygon(
      ctx,
      graphics::DrawMode::fill(),
      &[
        Point2::new(0.0, arrow_height/2.0),
        Point2::new(-arrow_width/2.0, -arrow_height/2.0),
        Point2::new(arrow_width/2.0, -arrow_height/2.0),
      ],
      graphics::Color::from_rgb(0, 32, 192)
    ).unwrap();

    Assets {
      font: font,

      char1: char1,
      char2: char2,
      monster: monster,

      sky_anim: Rc::new(sky_anim),
      grass_anim: Rc::new(grass_anim),
      rocks_anim: Rc::new(rocks_anim),
      dirt_anim: Rc::new(dirt_anim),

      left_tree_anim: Rc::new(left_tree_anim),
      right_tree_anim: Rc::new(right_tree_anim),
      left_bush_anim: Rc::new(left_bush_anim),
      right_bush_anim: Rc::new(right_bush_anim),
      wind_anim: Rc::new(wind_anim),

      after_attack_effect: after_attack_effect,

      button_width: button_width,
      button_margin: button_margin,

      cursor: cursor,
      button: button,

      music_bar_height: music_bar_height,
      music_bar: music_bar,

      now_line_width: now_line_width,
      now_line_x_offset: now_line_x_offset,
      now_line: now_line,

      measure_line: measure_line,
      measure_action_indicator: measure_action_indicator,

      arrow_width: arrow_width,
      up_arrow: up_arrow,
      down_arrow: down_arrow,
    }
  }
}
