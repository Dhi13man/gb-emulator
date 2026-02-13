mod renderer;
mod audio;
mod input;

use std::env;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gb_core::GameBoy;
use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::KeyCode;
use winit::window::{Window, WindowId};

const SCALE: u32 = 4;
const WIDTH: u32 = 160;
const HEIGHT: u32 = 144;
const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706); // ~59.73 Hz

struct App {
    rom_path: String,
    gb: Option<GameBoy>,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    audio: Option<audio::AudioPlayer>,
    last_frame: Instant,
}

impl App {
    fn new(rom_path: String) -> Self {
        Self {
            rom_path,
            gb: None,
            window: None,
            pixels: None,
            audio: None,
            last_frame: Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Create window
        let size = LogicalSize::new(WIDTH * SCALE, HEIGHT * SCALE);
        let attrs = Window::default_attributes()
            .with_title("GB Emulator")
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(WIDTH, HEIGHT));

        let window = Arc::new(event_loop.create_window(attrs).expect("Failed to create window"));
        let window_size = window.inner_size();

        // Create pixels surface (pixels 0.15 takes Arc<Window>)
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, Arc::clone(&window));
        let pixels = Pixels::new(WIDTH, HEIGHT, surface_texture)
            .expect("Failed to create pixels");

        // Load ROM
        let rom = fs::read(&self.rom_path).expect("Failed to read ROM file");
        let gb = GameBoy::new(rom);

        // Create audio player
        let audio_player = audio::AudioPlayer::new();

        self.window = Some(window);
        self.pixels = Some(pixels);
        self.gb = Some(gb);
        self.audio = audio_player.ok();
        self.last_frame = Instant::now();

        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(gb) = &mut self.gb {
                    let pressed = event.state.is_pressed();
                    if let Some(button) = input::map_key(event.physical_key) {
                        gb.set_button(button, pressed);
                    }

                    // Escape to quit
                    if pressed && event.physical_key == winit::keyboard::PhysicalKey::Code(KeyCode::Escape) {
                        event_loop.exit();
                    }
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(pixels) = &mut self.pixels {
                    let _ = pixels.resize_surface(size.width, size.height);
                }
            }

            WindowEvent::RedrawRequested => {
                if let (Some(gb), Some(pixels)) = (&mut self.gb, &mut self.pixels) {
                    // Run one frame
                    gb.run_frame();

                    // Render frame buffer to pixels
                    renderer::render(gb.frame_buffer(), pixels.frame_mut());

                    // Submit audio samples
                    if let Some(audio) = &mut self.audio {
                        let samples = gb.audio_buffer();
                        audio.push_samples(&samples);
                    }

                    // Present
                    if let Err(e) = pixels.render() {
                        eprintln!("Render error: {e}");
                        event_loop.exit();
                    }
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Frame pacing
        let elapsed = self.last_frame.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
        self.last_frame = Instant::now();

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <rom_file>", args[0]);
        std::process::exit(1);
    }

    let rom_path = args[1].clone();
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new(rom_path);
    event_loop.run_app(&mut app).expect("Event loop failed");
}
