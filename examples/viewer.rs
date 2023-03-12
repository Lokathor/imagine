use core::mem::size_of;

use beryllium::{
  events::Event,
  init::InitFlags,
  video::{CreateWinArgs, GlContextFlags, GlProfile, GlSwapInterval},
  Sdl,
};
use bytemuck::cast_slice;
use ezgl::{
  gl_constants::{GL_COLOR_BUFFER_BIT, GL_TRIANGLES, GL_UNSIGNED_INT},
  BufferTarget::*,
  BufferUsageHint::*,
  EzGl, MagFilter, MinFilter,
  ShaderType::*,
  TextureTarget::*,
  TextureWrap,
};
use imagine::{image::Bitmap, pixel_formats::RGBA8888};

const USE_GLES: bool = cfg!(target_arch = "aarch64") || cfg!(target_arch = "arm");

const GL_SHADER_HEADER: &str = "#version 410
";

const GLES_SHADER_HEADER: &str = "#version 310 es
precision mediump float;
";

const VERTEX_SRC: &str = "
  layout (location = 0) in vec3 position;
  layout (location = 1) in vec2 tex_coord_in;
  
  out vec2 fragment_tex_coord;
  
  void main() {
    gl_Position = vec4(position, 1.0);
    fragment_tex_coord = tex_coord_in;
  }";

const FRAGMENT_SRC: &str = "
  out vec4 fragment_color;

  in vec2 fragment_tex_coord;
  
  uniform sampler2D our_tex;
  
  void main() {
    fragment_color = texture(our_tex, fragment_tex_coord);
  }";

fn main() {
  // Generic args and file opening stuff
  let args: Vec<String> = std::env::args().collect();
  println!("ARGS: {args:?}");
  if args.len() < 2 {
    println!("run this with a filename to try and open that file.");
    return;
  }
  let path = std::path::Path::new(&args[1]);
  print!("Reading `{}`... ", path.display());
  let bytes = match std::fs::read(path) {
    Ok(bytes) => {
      println!("got {} bytes.", bytes.len());
      bytes
    }
    Err(e) => {
      println!("{e:?}");
      return;
    }
  };

  // THIS IS THE COOL PART WHERE WE'RE USING THE LIBRARY TO PARSE A FILE
  let mut image = match Bitmap::<RGBA8888>::try_from_png_bytes(&bytes) {
    Some(image) => image,
    None => match Bitmap::<RGBA8888>::try_from_bmp_bytes(&bytes).ok() {
      Some(image) => image,
      None => match Bitmap::<RGBA8888>::try_from_netpbm_bytes(&bytes).ok() {
        Some(image) => image,
        None => {
          println!("Couldn't parse the file.");
          return;
        }
      },
    },
  };
  image.vertical_flip();

  // Initializes SDL2
  let sdl = Sdl::init(InitFlags::VIDEO);
  if USE_GLES {
    // When on Aarch64 or ARM, assume that we're building for some sort of
    // raspberry pi situation and use GLES-3.1 (best available on pi)
    sdl.set_gl_profile(GlProfile::ES).unwrap();
    sdl.set_gl_context_major_version(3).unwrap();
    sdl.set_gl_context_minor_version(1).unwrap();
  } else {
    // For "normal" desktops we will use GL-4.1, which is the best that Mac can
    // offer.
    sdl.set_gl_profile(GlProfile::Core).unwrap();
    sdl.set_gl_context_major_version(4).unwrap();
    sdl.set_gl_context_minor_version(1).unwrap();
  }
  // optimistically assume that we can use multisampling.
  sdl.set_gl_multisample_buffers(1).unwrap();
  sdl.set_gl_multisample_count(if USE_GLES { 4 } else { 8 }).unwrap();
  sdl.set_gl_framebuffer_srgb_capable(true).unwrap();
  let mut flags = GlContextFlags::default();
  if cfg!(target_os = "macos") {
    flags |= GlContextFlags::FORWARD_COMPATIBLE;
  }
  if cfg!(debug_assertions) {
    flags |= GlContextFlags::DEBUG;
  }
  sdl.set_gl_context_flags(flags).unwrap();

  // Makes the window with a GL Context.
  let win = sdl
    .create_gl_window(CreateWinArgs {
      title: "Example GL Window",
      width: image.width.try_into().unwrap(),
      height: image.height.try_into().unwrap(),
      ..Default::default()
    })
    .unwrap();
  win.set_swap_interval(GlSwapInterval::AdaptiveVsync).ok();
  let gl = {
    let mut temp = EzGl::new_boxed();
    unsafe { temp.load(|name| win.get_proc_address(name)) }
    temp
  };
  if cfg!(debug_assertions) && win.supports_extension("GL_KHR_debug") {
    gl.set_stderr_debug_message_callback().ok();
  }

  if !USE_GLES {
    gl.enable_multisample(true);
    gl.enable_framebuffer_srgb(true);
  }
  gl.set_pixel_store_unpack_alignment(1);
  gl.set_clear_color(0.2, 0.3, 0.3, 1.0);

  let vao = gl.gen_vertex_array().unwrap();
  gl.bind_vertex_array(&vao);

  type Vertex = [f32; 5];
  let vertices: &[Vertex] = &[
    // positions    // texture coords
    [1.0, 1.0, 0.0, 1.0, 1.0],   // top right
    [1.0, -1.0, 0.0, 1.0, 0.0],  // bottom right
    [-1.0, -1.0, 0.0, 0.0, 0.0], // bottom left
    [-1.0, 1.0, 0.0, 0.0, 1.0],  // top left
  ];
  type TriElement = [u32; 3];
  let indices: &[TriElement] = &[[0, 1, 3], [1, 2, 3]];

  let vbo = gl.gen_buffer().unwrap();
  gl.bind_buffer(ArrayBuffer, &vbo);
  gl.alloc_buffer_data(ArrayBuffer, cast_slice(vertices), StaticDraw);

  let ebo = gl.gen_buffer().unwrap();
  gl.bind_buffer(ElementArrayBuffer, &ebo);
  gl.alloc_buffer_data(ElementArrayBuffer, cast_slice(indices), StaticDraw);

  gl.enable_vertex_attrib_array(0);
  gl.vertex_attrib_f32_pointer::<[f32; 3]>(0, size_of::<Vertex>(), size_of::<[f32; 0]>());
  gl.enable_vertex_attrib_array(1);
  gl.vertex_attrib_f32_pointer::<[f32; 2]>(1, size_of::<Vertex>(), size_of::<[f32; 3]>());

  let shader_header = if USE_GLES { GLES_SHADER_HEADER } else { GL_SHADER_HEADER };
  let vertex_shader = gl.create_shader(VertexShader).unwrap();
  let vertex_src = format!("{shader_header}\n{VERTEX_SRC}");
  gl.set_shader_source(&vertex_shader, &vertex_src);
  gl.compile_shader(&vertex_shader);
  if !gl.get_shader_compile_success(&vertex_shader) {
    let log = gl.get_shader_info_log(&vertex_shader);
    panic!("Vertex Shader Error: {log}");
  }

  let fragment_shader = gl.create_shader(FragmentShader).unwrap();
  let fragment_src = format!("{shader_header}\n{FRAGMENT_SRC}");
  gl.set_shader_source(&fragment_shader, &fragment_src);
  gl.compile_shader(&fragment_shader);
  if !gl.get_shader_compile_success(&fragment_shader) {
    let log = gl.get_shader_info_log(&fragment_shader);
    panic!("Vertex Shader Error: {log}");
  }

  let program = gl.create_program().unwrap();
  gl.attach_shader(&program, &vertex_shader);
  gl.attach_shader(&program, &fragment_shader);
  gl.link_program(&program);
  if !gl.get_program_link_success(&program) {
    let log = gl.get_program_info_log(&program);
    panic!("Program Link Error: {log}");
  }
  gl.use_program(&program);

  let texture = gl.gen_texture().unwrap();
  gl.bind_texture(Texture2D, &texture);
  gl.set_texture_wrap_s(Texture2D, TextureWrap::MirroredRepeat);
  gl.set_texture_wrap_t(Texture2D, TextureWrap::MirroredRepeat);
  gl.set_texture_border_color(Texture2D, &[1.0, 1.0, 0.0, 1.0]);
  gl.set_texture_min_filter(Texture2D, MinFilter::LinearMipmapLinear);
  gl.set_texture_mag_filter(Texture2D, MagFilter::Linear);
  gl.alloc_tex_image_2d(
    Texture2D,
    0,
    image.width.try_into().unwrap(),
    image.height.try_into().unwrap(),
    cast_slice(&image.pixels),
  );
  gl.generate_mipmap(Texture2D);

  // program "main loop".
  'the_loop: loop {
    // Process events from this frame.
    #[allow(clippy::never_loop)]
    #[allow(clippy::single_match)]
    while let Some((event, _timestamp)) = sdl.poll_events() {
      match event {
        Event::Quit => break 'the_loop,
        _ => (),
      }
    }

    gl.clear(GL_COLOR_BUFFER_BIT);
    unsafe { gl.draw_elements(GL_TRIANGLES, 6, GL_UNSIGNED_INT, 0) };

    win.swap_window();
  }
}
