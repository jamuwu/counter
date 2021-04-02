use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::{PhysicalPosition, LogicalSize};
use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use image::{GenericImage, GenericImageView, Rgba};
use scrap::{Capturer, Display};


#[cfg(windows)]
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
  let mut input = WinitInputHelper::new();
  let window = WindowBuilder::new()
    .with_decorations(false)
    .with_transparent(true)
    .with_inner_size(LogicalSize::new(300.0, 100.0))
    .with_resizable(false)
    .with_always_on_top(true)
    .build(&event_loop)
    .unwrap();

  let w = window.primary_monitor().unwrap().size().width / 2 - 100;
  window.set_outer_position(PhysicalPosition {x: w, y: 10});

  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(300, 100, surface_texture)?
  };
  let mut app = Counter::new();

  event_loop.run(move |event, _, control_flow| {
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

    if input.update(&event) {
      app.update();
      window.request_redraw();
    }
  });
}

struct Template {
  pixels: Vec<u8>,
  width: i16,
  height: i16,
}

struct Counter {
  tmpl: Option<Template>,
  actv: bool,
  bar_x: i16,
  bar_y: i16,
  bar_w: i16,
  bar_h: i16,
}

impl Counter {
  fn new() -> Self {
    Self {
      tmpl: None,
      actv: false,
      bar_x: 0,
      bar_y: 0,
      bar_w: 0,
      bar_h: 0,
    }
  }

  fn get_cropped(&self, frame: &[u8], n: &mut Vec<u8>) {
    for (i, p) in frame.chunks_exact(4).enumerate() {
      let x = (i % self.tmpl.as_ref().unwrap().height as usize) as i16;
      let y = (i / self.tmpl.as_ref().unwrap().width as usize) as i16;

      if x >= self.bar_x && x < self.bar_x + self.bar_h
        && y >= self.bar_y && y < self.bar_y + self.bar_w {
        for c in p {
          n.push(*c);
        }
      }
    }
  }

  fn diff(&self, frame: &mut [u8]) -> f64 {
    let mut score = 0;
    let w = self.tmpl.as_ref().unwrap().width;
    let h = self.tmpl.as_ref().unwrap().height;
    let px = &self.tmpl.as_ref().unwrap().pixels;
    let fx = frame.chunks_exact(4);
    for (p1, p2) in px.chunks_exact(4).zip(fx) {
      score += (p1[0] as i32 - p2[0] as i32).abs() +
        (p1[1] as i32 - p2[1] as i32).abs() +
        (p1[2] as i32 - p2[2] as i32).abs()
    }
    return score as f64 * 100.0 / (255.0 * 3.0 * (w * h) as f64)
  }

  fn update(&mut self) {
    let x = get_active_window();
    if x == "PokeMMO" {
      if !self.actv {
        self.actv = true;
        println!("PokeMMO Refocused.");
      }
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
      pixel.copy_from_slice(&[0x5e, 0x48, 0xe8, 0xff]);
    }
  }
}