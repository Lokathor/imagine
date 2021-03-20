use fermium::{error::*, events::*, video::*, *};
use gl33::*;

fn get_err_msg_and_panic() {
  unsafe {
    let mut v = Vec::with_capacity(4096);
    let mut p = SDL_GetErrorMsg(v.as_mut_ptr(), v.capacity() as _);
    while *p != 0 {
      print!("{}", *p as u8 as char);
      p = p.add(1);
    }
    println!();
    panic!();
  }
}

fn main() {
  // TODO: read a PNG filename to display from the command line.

  unsafe {
    assert_eq!(0, SDL_Init(SDL_INIT_VIDEO));
    assert_eq!(0, SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 3));
    assert_eq!(0, SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 3));
    assert_eq!(
      0,
      SDL_GL_SetAttribute(
        SDL_GL_CONTEXT_PROFILE_MASK,
        SDL_GL_CONTEXT_PROFILE_CORE.0 as _
      )
    );

    // TODO: the window size should be based on the PNG file we're reading.
    // TODO: window title should be based on the PNG file we're reading.

    let win = SDL_CreateWindow(
      b"imagine fermium demo\0".as_ptr().cast(),
      50,
      50,
      800,
      600,
      (SDL_WINDOW_SHOWN | SDL_WINDOW_OPENGL).0 as _,
    );
    if win.is_null() {
      get_err_msg_and_panic();
    }
    let ctx = SDL_GL_CreateContext(win);
    if ctx.0.is_null() {
      get_err_msg_and_panic();
    }
    let gl =
      GlFns::load_from(&|c_char_ptr| SDL_GL_GetProcAddress(c_char_ptr.cast())).unwrap();
    gl.ClearColor(0.2, 0.3, 0.3, 1.0);

    // TODO: setup for displaying an image

    let mut event: SDL_Event = core::mem::zeroed();
    'program: loop {
      while SDL_PollEvent(&mut event) != 0 {
        if event.common.type_ == SDL_QUIT as _ {
          break 'program;
        }
      }

      gl.Clear(GL_COLOR_BUFFER_BIT);

      // TODO: draw the image

      SDL_GL_SwapWindow(win);
    }
    SDL_DestroyWindow(win);
    SDL_Quit();
  }
}
