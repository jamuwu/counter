use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::{PhysicalPosition, LogicalSize};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use scrap::{Capturer, Display};
use rusttype::{point, Font, Scale};

mod pokemon;
use pokemon::Pokemon;

mod gif;
use gif::Gif;

#[cfg(target_os = "linux")]
fn get_active_window() -> String {
  // How do I do the same on linux?
  String::from("PokeMMO Counter")
}

#[cfg(target_os = "windows")]
fn get_active_window() -> String {
  let x = unsafe {winapi::um::winuser::GetForegroundWindow()};
  let mut b = [0u16; 1024];
  let mut c = Vec::new();
  unsafe {winapi::um::winuser::GetWindowTextW(x, b.as_mut_ptr(), 1024)};
  let mut i = 0;
  loop {
    c.push(match b[i] {
      1056 => 80,
      1086 => 111,
      1077 => 101,
      1052 => 77,
      1054 => 79,
      _ => b[i],
    });
    i += 1;
    if i == 1024 {
      break;
    }
  }
  let pos = c.iter().position(|&c| c==0).expect("LPWSTR is not null terminated");
  String::from_utf16_lossy(&c[..pos])
}

fn main() -> Result<(), Error> {
  env_logger::init();
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_decorations(false)
    .with_transparent(true)
    .with_inner_size(LogicalSize::new(300.0, 100.0))
    .with_resizable(false)
    .with_always_on_top(true)
    .with_title("PokeMMO Counter")
    .build(&event_loop)
    .unwrap();

  let w = window.primary_monitor().unwrap().size().width - 600;
  window.set_outer_position(PhysicalPosition {x: w, y: 24});

  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(300, 100, surface_texture)?
  };
  let mut app = Counter::new();

  let mut visible = true;

  event_loop.run(move |event, _, control_flow| {
    if let Event::WindowEvent{event: WindowEvent::CloseRequested, ..} = event {
      *control_flow = ControlFlow::Exit;
      return;
    }

    if let Event::RedrawRequested(_) = event {
      app.draw(pixels.get_frame());
      if pixels
        .render()
        .map_err(|e| error!("pixels.render() failed: {}", e))
        .is_err()
      {
        *control_flow = ControlFlow::Exit;
        return;
      }
    }

    app.update();
    window.request_redraw();
    if app.actv != visible {
      visible = app.actv;
      window.set_minimized(!app.actv);
    }
  });
}

#[derive(Copy, Clone)]
struct Color {
  r: u8,
  g: u8,
  b: u8,
  a: u8,
}

impl Color {
  fn compare(&self, p: &[u8]) -> bool {
    let r = self.r as i16 <= p[2] as i16 + 10 && self.r as i16 >= p[2] as i16 - 10;
    let g = self.g as i16 <= p[1] as i16 + 10 && self.g as i16 >= p[1] as i16 - 10;
    let b = self.b as i16 <= p[0] as i16 + 10 && self.b as i16 >= p[0] as i16 - 10;
    return r && g && b
  }

  fn blend(&self, p: &[u8]) -> [u8; 4] {
    // https://github.com/abonander/rust-image/blob/master/src/color.rs#L420
    let (br, bg, bb, ba) = (self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0, self.a as f32 / 255.0);
    let (fr, fg, fb, fa) = (p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0);
    let alpha_final = ba + fa - ba * fa;
    let (br_a, bg_a, bb_a) = (br * ba, bg * ba, bb * ba);
    let (fr_a, fg_a, fb_a) = (fr * fa, fg * fa, fb * fa);
    let (out_r_a, out_g_a, out_b_a) = (fr_a + br_a * (1.0 - fa), fg_a + bg_a * (1.0 - fa), fb_a + bb_a * (1.0 - fa));
    let (out_r, out_g, out_b) = (out_r_a / alpha_final, out_g_a / alpha_final, out_b_a / alpha_final);
    [(255.0 * out_r) as u8, (255.0 * out_g) as u8, (255.0 * out_b) as u8, (255.0 * alpha_final) as u8]
  }
}

struct Counter<'a> {
  actv: bool,
  color: Color,
  count: usize,
  height: usize,
  width: usize,
  low: usize,
  mon: Pokemon,
  gif: Gif,
  encounters: u128,
  font: Font<'a>,
  capturer: Capturer,
}

impl Counter<'_> {
  fn new() -> Self {
    let display = Display::primary().expect("Couldn't find primary display.");
    let mon = Pokemon::Charizard;
    Self {
      actv: false,
      color: Color{r: 132, g: 209, b: 142, a: 255},
      count: 0,
      height: 1080,
      width: 1920,
      low: 1080,
      mon: mon,
      gif: Gif::open(mon),
      encounters: 0,
      font: Font::try_from_bytes(include_bytes!("Righteous.ttf")).expect("Couldn't load font."),
      capturer: Capturer::new(display).expect("Couldn't begin capture."),
    }
  }

  fn update(&mut self) {
    let x = get_active_window();
    if x == "PokeMMO" || x == "PokeMMO Counter" {
      self.gif.update();
      if !self.actv {
        self.actv = true;
        println!("PokeMMO Refocused.");
      }  
      let buffer = match self.capturer.frame() {
        Ok(buffer) => buffer,
        Err(error) => {
          if error.kind() == std::io::ErrorKind::WouldBlock {
            return;
          } else {
            panic!("Error: {}", error);
          }
        }
      };
      let new = detect(&buffer, self.color, self.width, self.height, self.low);
      if new.len() != self.count {
        if self.count != 0 && new.len() != 0 && self.low == self.height {
          self.low = new.iter().map(|x| x[1]).max().unwrap_or(0) - 10;
        } else {
          self.count = new.len();
          self.encounters += new.len() as u128;
        }
      }
    } else {
      if self.actv {
        self.actv = false;
        println!("PokeMMO Unfocused.");
      }
    }
    // TODO handle clicking on gif, left click to increment, right click to decrement
    // TODO if the pokemon name is clicked, open scrollable menu to change current pokemon
  }

  fn draw(&self, frame: &mut [u8]) {
    for pixel in frame.chunks_exact_mut(4) {
      pixel.copy_from_slice(&[0, 0, 0, 0]);
    }
    self.text(frame, "Encounters", 18.0, 180, 5);
    self.text(frame, &format!("{}", self.encounters), 32.0, 180, 17);
    self.text(frame, "Insanity", 18.0, 180, 44);
    self.text(frame, &format!("{:.2}%", insanity(self.encounters)),32.0, 180, 56);
    let mon = self.gif.frame();
    let offset_y = (100 - mon.height() as usize) / 2;
    let offset_x = 300 - mon.width() as usize;
    let mut x = 0;
    let mut y = 0;
    for p in mon.pixels() {
      if x < mon.width() as usize {
        let i = ((y + offset_y) * 300 + x + offset_x) * 4;
        frame[i..i + 4].copy_from_slice(
          &Color{r: 0, g: 0, b: 0, a: 0}.blend(&[p[0], p[1], p[2], p[3]])
        );
        x += 1;
      } else {
        x = 1;
        y += 1;
      }
    }
  }

  fn text(&self, frame: &mut [u8], string: &str, scale: f32, offset_x: usize, offset_y: usize) {
    let scale = Scale::uniform(scale);
    let v_metrics = self.font.v_metrics(scale);
    let glyphs: Vec<_> = self.font
      .layout(string, scale, point(5.0, 5.0 + v_metrics.ascent))
      .collect();
    let gw = {
      let min_x = glyphs
        .first()
        .map(|g| g.pixel_bounding_box().unwrap().min.x)
        .unwrap();
      let max_x = glyphs
        .last()
        .map(|g| g.pixel_bounding_box().unwrap().max.x)
        .unwrap();
      (max_x - min_x) as usize
    };
    for glyph in glyphs {
      if let Some(bounding_box) = glyph.pixel_bounding_box() {
        glyph.draw(|x, y, v| {
          if v > 0.1 {
            let x = x as usize + bounding_box.min.x as usize + (offset_x - gw);
            let y = y as usize + bounding_box.min.y as usize + offset_y;
            let i = (y * 300 + x) * 4;
            frame[i..i + 4].copy_from_slice(
              &Color{r: 0, g: 0, b: 0, a: 255}.blend(&[0xFF, 0xFF, 0xFF, (v * 255.0) as u8])
            );
          }
        });
      }
    }
  }
}

fn detect(frame: &scrap::Frame, c: Color, w: usize, h: usize, l: usize) -> Vec<[usize; 2]> {
  let mut frame = frame.to_vec();
  let mut n: Vec<[usize; 2]> = vec![];
  let mut y = 0;
  let mut count = 0;
  let s = frame.len() / h;
  while y < h && y < l{
    let prev = n.len();
    let mut found = false;
    let mut x = 0;
    while x < w {
      let i = s * y + 4 * x;
      if c.compare(&frame[i..i+3]) {
        frame[i] = 0;
        frame[i+1] = 0;
        frame[i+2] = 0;
        if !found {
          count = 1;
          found = true;
        } else {
          count += 1;
        }
      } else {
        if found {
          if count >= 50 {
            n.push([x, y]);
          }
        }
        found = false;
        count = 0;
      }
      x += 1;
    }
    if prev != n.len() {
      y += 5;
    } else {
      y += 1;
    }
  }
  // println!("Got {} matches", n);
  n
}

fn insanity(e: u128) -> f64 {
  (1.0 - (1.0 - (1.0 / 30000.0f64)).powf(e as f64)) * 100.0
}