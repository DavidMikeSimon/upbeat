use ggez::{graphics, Context};

pub struct Assets {
  pub font: graphics::Font,

  pub bg: graphics::Image,

  pub music_bar_height: f32,
  pub music_bar: graphics::Mesh,

  pub command_window_width: f32,
  pub command_window: graphics::Mesh,

  pub command_cursor: graphics::Mesh,

  pub now_line_width: f32,
  pub now_line_x_offset: f32,
  pub now_line: graphics::Mesh,

  pub arrow_width: f32,
  pub up_arrow: graphics::Mesh,
  pub down_arrow: graphics::Mesh,
}

impl Assets {
  pub fn new(ctx: &mut Context) -> Assets {
    let font = graphics::Font::new(ctx, "/fonts/Catamaran/Catamaran-Regular.ttf").unwrap();

    let bg = graphics::Image::new(ctx, "/images/bg.jpg").unwrap();

    let window = graphics::screen_coordinates(ctx);

    let music_bar_height = 200.0;
    let command_window_width = 300.0;

    let music_bar = graphics::Mesh::new_rectangle(
      ctx,
      graphics::DrawMode::fill(),
      graphics::Rect::new(0.0, 0.0, window.w - command_window_width, music_bar_height),
      graphics::Color::from_rgb(210, 210, 210)
    ).unwrap();

    let command_window = graphics::Mesh::new_rectangle(
      ctx,
      graphics::DrawMode::fill(),
      graphics::Rect::new(0.0, 0.0, command_window_width, music_bar_height),
      graphics::Color::from_rgb(192, 192, 192)
    ).unwrap();

    let now_line_width = 2.0;
    let now_line_x_offset = 100.0;

    let now_line = graphics::Mesh::new_line(
      ctx,
      &[
        nalgebra::Point2::new(0.0, 0.0),
        nalgebra::Point2::new(0.0, music_bar_height)
      ],
      now_line_width,
      graphics::BLACK
    ).unwrap();


    let arrow_width = 20.0;
    let arrow_height = 10.0;

    let up_arrow = graphics::Mesh::new_polygon(
      ctx,
      graphics::DrawMode::fill(),
      &[
        nalgebra::Point2::new(0.0, -arrow_height/2.0),
        nalgebra::Point2::new(arrow_width/2.0, arrow_height/2.0),
        nalgebra::Point2::new(-arrow_width/2.0, arrow_height/2.0),
      ],
      graphics::Color::from_rgb(0, 192, 32)
    ).unwrap();

    let down_arrow = graphics::Mesh::new_polygon(
      ctx,
      graphics::DrawMode::fill(),
      &[
        nalgebra::Point2::new(0.0, arrow_height/2.0),
        nalgebra::Point2::new(-arrow_width/2.0, -arrow_height/2.0),
        nalgebra::Point2::new(arrow_width/2.0, -arrow_height/2.0),
      ],
      graphics::Color::from_rgb(0, 32, 192)
    ).unwrap();

    let command_cursor_width = 30.0;
    let command_cursor = graphics::Mesh::new_polygon(
      ctx,
      graphics::DrawMode::fill(),
      &[
        nalgebra::Point2::new(0.0, -command_cursor_width/2.0),
        nalgebra::Point2::new(command_cursor_width/2.0, 0.0),
        nalgebra::Point2::new(0.0, command_cursor_width/2.0),
        nalgebra::Point2::new(-command_cursor_width/2.0, 0.0),
      ],
      graphics::Color::from_rgb(192, 32, 192)
    ).unwrap();

    Assets {
      font: font,

      bg: bg,

      music_bar_height: music_bar_height,
      music_bar: music_bar,

      command_window_width: command_window_width,
      command_window: command_window,

      command_cursor: command_cursor,

      now_line_width: now_line_width,
      now_line_x_offset: now_line_x_offset,
      now_line: now_line,

      arrow_width: arrow_width,
      up_arrow: up_arrow,
      down_arrow: down_arrow,
    }
  }
}
