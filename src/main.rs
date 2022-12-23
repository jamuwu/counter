use bincode::{serialize, deserialize};
use fontdue::{
  Font, FontSettings,
  layout::{
    CoordinateSystem,
    HorizontalAlign,
    Layout, LayoutSettings,
    TextStyle,
  },
};
use image::{
  AnimationDecoder,
  codecs::gif::GifDecoder,
  imageops::FilterType::Gaussian,
  Pixel, Rgba,
};
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use regex::Regex;
use std::fs::read;
use std::io::Cursor;
use std::time::{Duration, Instant};
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use win_screenshot::addon::*;
use win_screenshot::capture::*;

mod pokemon;
use pokemon::Pokemon;

lazy_static::lazy_static!(
  static ref RE: Regex = Regex::new(r"[P|Р][o][k][e|е][M|М][M|М][O]").unwrap();
);

fn main() -> Result<(), Error> {
  env_logger::init();
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_inner_size(LogicalSize::new(300.0, 100.0))
    .with_always_on_top(true)
    .with_decorations(false)
    .with_resizable(false)
    .with_transparent(true)
    .with_title("Encounter Counter")
    .build(&event_loop)
    .unwrap();
    
  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(300, 100, surface_texture)?
  };
  pixels.set_clear_color(pixels::wgpu::Color::TRANSPARENT);
    
  let mut app = App::new();
    
  window.set_outer_position(app.pos);

  window.request_redraw();
  event_loop.run(move |event, _, flow| {
    if let Event::WindowEvent{event: WindowEvent::CloseRequested, ..} = event {
      *flow = ControlFlow::Exit;
      return;
    }

    if let Event::WindowEvent {
      event: WindowEvent::MouseInput {
        state: ElementState::Pressed,
        button: MouseButton::Left,
        ..
      },
      ..
    } = event {
      window.drag_window().unwrap();
    }
    
    if let Event::WindowEvent {
      event: WindowEvent::Moved(pos),
      ..
    } = event {
      app.update_pos(pos);
    }
    
    if let Event::RedrawRequested(_) = event {
      //println!("Drawing");
      app.draw(pixels.get_frame_mut());
      if let Err(err) = pixels.render() {
        error!("pixels.render() failed: {err}");
        *flow = ControlFlow::Exit;
        return;
      }
    }
    
    if app.fps.tick() {
      //println!("Updating");
      app.update();
    }
    window.request_redraw();
  });

  Ok(())
}

struct App {
  db: sled::Db,
  fonts: Vec<Font>,
  hwnd: Option<isize>,
  fps: FPS,
  pos: PhysicalPosition<i32>,
  previous: usize,
  pokemon: Vec<Pokemon>,
  animation: Vec<Gif>,
  encounters: usize,
  adding: Option<usize>,
}

impl App {
  fn new() -> Self {
    let db = sled::open("data").expect("Could not open data");
    let pos: PhysicalPosition<i32> = if let Ok(Some(p)) = &db.get(b"position") {
      deserialize(p.as_ref()).unwrap()
    } else {
      PhysicalPosition { x: 200, y: 200 }
    };
    let encounters: usize = if let Ok(Some(p)) = db.get(b"encounters") {
      usize::from_le_bytes(
        p.as_ref().try_into().expect("slice with incorrect length")
      )
    } else {
      0
    };
    Self {
      db: db,
      fonts: vec![Font::from_bytes(
        include_bytes!("Righteous.ttf") as &[u8],
        FontSettings::default()
      ).expect("Couldn't load font.")],
      hwnd: None,
      fps: FPS::new(30),
      pos: pos,
      previous: 0,
      pokemon: vec![Pokemon::Poliwag],
      animation: vec![Gif::open(Pokemon::Poliwag)],
      encounters: encounters,
      adding: Some(0),
    }
  }

  fn update(&mut self) {
    for gif in self.animation.iter_mut() {
      gif.update();
    }
    if let Some(hwnd) = self.hwnd {
      if let Ok(screen) = capture_window(hwnd, Area::ClientOnly) {
        let mut lines = vec![];
        let mut skip = 0;

        for (y, row) in screen.rows().enumerate() {
          if skip > 0 {
            skip -= 1;
            continue;
          }

          let mut count = 0;
          let mut prev = false;
          for (x, pixel) in row.enumerate() {
            if similar(pixel, Rgba::from_slice(&[131, 205, 140, 255])) {
              count += 1;
              if count >= 100 && prev{
                lines.push((x, y));
                count = 0;
                skip = 10;
              }
              prev = true;
            } else {
              prev = false;
            }
          }
        }

        if self.previous == 0 {
          match lines.len() {
            1 | 2 => self.adding = Some(1),
            3 | 4 => self.adding = Some(3),
            5 | 6 => self.adding = Some(5),
            _ => (),
          }
        }
        self.previous = lines.len();
      } else {
        self.update_hwnd();
      }
    } else {
      self.update_hwnd();
    }
  }

  fn draw(&mut self, frame: &mut [u8]) {
    if let Some(add) = self.adding {
      self.adding = None;
      self.update_encounters(add);
      println!("Encounters updated to {}", self.encounters);
      frame.copy_from_slice(&[0, 0, 0, 60].repeat(frame.len() / 4));
      let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
      layout.reset(&LayoutSettings {
        horizontal_align: HorizontalAlign::Right,
        ..LayoutSettings::default()
      });
      layout.append(&self.fonts, &TextStyle::new("Encounters\n", 12.0, 0));
      layout.append(&self.fonts, &TextStyle::new(&format!("{}\n", self.encounters), 18.0, 0));
      layout.append(&self.fonts, &TextStyle::new("Insanity\n", 12.0, 0));
      layout.append(&self.fonts, &TextStyle::new(&format!("{:.2}%", insanity(self.encounters)), 18.0, 0));
      let text_width = if let Some(current) = layout.glyphs().last() {
        current.x as usize + current.width
      } else { 0 };
      for (n, glyph) in layout.glyphs().iter().enumerate() {
        if glyph.char_data.rasterize() {
          let (_, bitmap) = self.fonts[glyph.font_index].rasterize(glyph.parent, glyph.key.px);
          for y in 0..glyph.height {
            for x in 0..glyph.width {
              let mut c = Rgba::from([0, 0, 0, 0]);
              c.blend(&Rgba::from_slice(&[255, 255, 255, bitmap[y * glyph.width + x]]));
              let _x = 180 - text_width + glyph.x as usize + x;
              let _y = glyph.y as usize + y + 5;
              let i = (_y * 300 + _x) * 4;
              frame[i..i + 4].copy_from_slice(c.channels());
            }
          }
        }
      }
    }
    //self.text(frame, "Encounters\n", 18.0, 180, 5);
    //self.text(frame, &format!("{}", self.encounters), 32.0, 180, 17);
    //self.text(frame, "Insanity", 18.0, 180, 44);
    //self.text(frame, &format!("{:.2}%", insanity(self.encounters)),32.0, 180, 56);
    for (i, gif) in self.animation.iter().enumerate() {
      let image = gif.frame();
      let offset_y = (100 * (i + 1) - image.height() as usize) / 2;
      let offset_x = 300 - image.width() as usize;
      for (y, row) in image.rows().enumerate() {
        for (x, pixel) in row.enumerate() {
          let i = ((y + offset_y) * 300 + x + offset_x) * 4;
          let mut c = Rgba::from([0, 0, 0, 60]);
          c.blend(&Rgba::from_slice(&[pixel[0], pixel[1], pixel[2], pixel[3]]));
          frame[i..i + 4].copy_from_slice(c.channels());
        }
      }
    }
  }

  fn update_hwnd(&mut self) {
    if let Some(win) = window_list().unwrap().iter().find(|i| RE.is_match(&i.window_name)) {
      self.hwnd = Some(win.hwnd);
    } else {
      error!("Cannot find PokeMMO window, is it open?");
    }
  }

  fn update_encounters(&mut self, n: usize) {
    self.encounters += n;
    if let Err(err) = self.db.insert(b"encounters", &self.encounters.to_le_bytes()) {
      error!("Could not update the encounters in data: {}", err);
    }
  } 

  fn update_pos(&mut self, pos: PhysicalPosition<i32>) {
    if let Err(err) = self.db.insert(b"position", serialize(&pos).unwrap()) {
      error!("Could not update the window position in data: {}", err);
    } else {
      println!("Updated pos");
    }
  }
}

struct Data {
  position: PhysicalPosition<u32>,
  encounters: Vec<(Pokemon, usize)>,
}

#[allow(deprecated)]
fn similar(c1: &Rgba<u8>, c2: &Rgba<u8>) -> bool {
  let (c1r, c1g, c1b, _) = c1.channels4();
  let (c2r, c2g, c2b, _) = c2.channels4();
  within(10, c1r, c2r) && within(10, c1g, c2g) && within(10, c1b, c2b)
}

fn within(r: u8, a: u8, b: u8) -> bool {
  if a > b {
      a - b <= r
  } else {
      b - a <= r
  }
}

fn insanity(e: usize) -> f64 {
  (1.0 - (1.0 - (1.0 / 30000.0f64)).powf(e as f64)) * 100.0
}

struct Gif {
  frames: Vec<(image::RgbaImage, image::Delay)>,
  current: usize,
  instant: Instant,
}

impl Gif {
  fn open(mon: crate::Pokemon) -> Self {
    Self {
      frames: {
        let buffer = read(&format!("gifs/{}.gif", mon as u16)).unwrap();
        let buffer = Cursor::new(buffer);
        let decoder = GifDecoder::new(buffer).unwrap();
        let frames = decoder.into_frames().collect_frames().unwrap();
        frames.iter().map(|f| (
          image::DynamicImage::ImageRgba8(f.clone().into_buffer())
            .resize(90, 90, Gaussian).to_rgba8(),
          f.delay()
        )).collect()
      },
      current: 0,
      instant: Instant::now(),
    }
  }

  fn update(&mut self) {
    let (numer, denom) = self.frames[self.current].1.numer_denom_ms();
    if self.instant.elapsed() >= Duration::from_millis((numer / denom).into()) {
      self.current += 1;
      self.current %= self.frames.len();
      self.instant = Instant::now();
    }
  }

  fn frame(&self) -> &image::RgbaImage {
    &self.frames[self.current].0
  }
}

struct FPS {
  length: Duration,
  last: Instant,
}

impl FPS {
  fn new(limit: u32) -> FPS {
    println!("{:?}", Duration::from_secs(1) / limit);
    FPS {
      length: Duration::from_secs(1) / limit,
      last: Instant::now(),
    }
  }

  fn tick(&mut self) -> bool {
    if self.last.elapsed() >= self.length {
      self.last = Instant::now();
      true
    } else {
      false
    }
  }
}