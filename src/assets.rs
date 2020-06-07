use ggez::{graphics, Context};

pub struct Assets {
  pub arrow_width: f32,
  pub up_arrow: graphics::Mesh,
  pub down_arrow: graphics::Mesh,
}

impl Assets {
  pub fn new(ctx: &mut Context) -> Assets {
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

    Assets {
      arrow_width: arrow_width,
      up_arrow: up_arrow,
      down_arrow: down_arrow,
    }
  }
}
