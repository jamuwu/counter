use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::{PhysicalPosition, LogicalSize};
use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use scrap::{Capturer, Display};


#[cfg(target_os = "linux")]
fn get_active_window() -> String {
  // Can't find how to get window
  // Titles on linux, so for now
  // We'll waste your cpu cycles
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

  let w = window.primary_monitor().unwrap().size().width - 300;
  window.set_outer_position(PhysicalPosition {x: w, y: 22});

  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(300, 100, surface_texture)?
  };
  let mut app = Counter::new();

  let mut visible = true;

  event_loop.run(move |event, _, control_flow| {
    if let Event::RedrawRequested(_) = event {
      if pixels
        .render()
        .map_err(|e| error!("pixels.render() failed: {}", e))
        .is_err()
      {
        *control_flow = ControlFlow::Exit;
        return;
      }
      app.draw(pixels.get_frame());
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
}

impl Color {
  fn compare(&self, p: &[u8]) -> bool {
    let r = self.r as i16 <= p[2] as i16 + 10 && self.r as i16 >= p[2] as i16 - 10;
    let g = self.g as i16 <= p[1] as i16 + 10 && self.g as i16 >= p[1] as i16 - 10;
    let b = self.b as i16 <= p[0] as i16 + 10 && self.b as i16 >= p[0] as i16 - 10;
    return r && g && b
  }
}

struct Counter {
  actv: bool,
  color: Color,
  count: u8,
  height: i16,
  width: i16,
  capturer: Capturer,
}

impl Counter {
  fn new() -> Self {
    let display = Display::primary().expect("Couldn't find primary display.");
    Self {
      actv: false,
      color: Color{r: 132, g: 209, b: 142},
      count: 0,
      height: 1080,
      width: 1920,
      capturer: Capturer::new(display).expect("Couldn't begin capture."),
    }
  }

  fn update(&mut self) {
    let x = get_active_window();
    if x == "PokeMMO" || x == "PokeMMO Counter" {
      if !self.actv {
        self.actv = true;
        println!("PokeMMO Refocused.");
      }  
      // Wait until there's a frame
      let buffer = match self.capturer.frame() {
        Ok(buffer) => buffer,
        Err(error) => {
          if error.kind() == std::io::ErrorKind::WouldBlock {
            // Keep spinning
            return;
          } else {
            panic!("Error: {}", error);
          }
        }
      };
      self.count = detect(&buffer, self.color, self.width as usize, self.height as usize);
    } else {
      if self.actv {
        self.actv = false;
        println!("PokeMMO Unfocused.");
      }
    }
    // TODO take screenshot and detect hp bars
    // TODO handle clicking on gif, but only the gif, if it's even a pixel off ignore that shit
  }

  fn draw(&self, frame: &mut [u8]) {
    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
      pixel.copy_from_slice(&[0x00, 0xFF, 0xC0, 0x0F]);
    }
  }
}

fn detect(frame: &scrap::Frame, c: Color, w: usize, h: usize) -> u8 {
  let mut frame = frame.to_vec();
  let mut n = 0;
  let mut y = 0;
  let mut count = 0;
  let s = frame.len() / h;
  while y < h {
    let prev = n;
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
          if count >= 100 {
            n += 1;
          }
        }
        found = false;
        count = 0;
      }
      x += 1;
    }
    if prev != n {
      y += 5;
    } else {
      y += 1;
    }
  }
  println!("Got {} matches", n);
  n
}
