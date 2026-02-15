#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Write;
use sf_nano_core::Instance;

// — Minimal allocator using libc malloc/free —

struct LibcAllocator;

unsafe impl alloc::alloc::GlobalAlloc for LibcAllocator {
    unsafe fn alloc(&self, layout: alloc::alloc::Layout) -> *mut u8 {
        unsafe { libc::malloc(layout.size()) as *mut u8 }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: alloc::alloc::Layout) {
        unsafe { libc::free(ptr as *mut libc::c_void) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, _layout: alloc::alloc::Layout, new_size: usize) -> *mut u8 {
        unsafe { libc::realloc(ptr as *mut libc::c_void, new_size) as *mut u8 }
    }
}

#[global_allocator]
static ALLOC: LibcAllocator = LibcAllocator;

// — Stderr writer —

struct Stderr;

impl Write for Stderr {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe { libc::write(2, s.as_ptr() as *const libc::c_void, s.len()) };
        Ok(())
    }
}

macro_rules! eprintln {
    ($($arg:tt)*) => {{
        let _ = writeln!(Stderr, $($arg)*);
    }};
}

// — Panic handler —

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        let msg = b"panic\n";
        libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
        libc::exit(101)
    }
}

#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

// — File reading via libc —

fn read_file(path: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let fd = libc::open(path.as_ptr() as *const libc::c_char, libc::O_RDONLY);
        if fd < 0 {
            return None;
        }

        // Get file size
        let end = libc::lseek(fd, 0, libc::SEEK_END);
        if end < 0 {
            libc::close(fd);
            return None;
        }
        libc::lseek(fd, 0, libc::SEEK_SET);

        let size = end as usize;
        let mut buf = vec![0u8; size];
        let mut read_total = 0;
        while read_total < size {
            let n = libc::read(fd, buf[read_total..].as_mut_ptr() as *mut libc::c_void, size - read_total);
            if n <= 0 {
                libc::close(fd);
                return None;
            }
            read_total += n as usize;
        }
        libc::close(fd);
        Some(buf)
    }
}

// — Entry point —

#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        eprintln!("Usage: sf-nano-cli-minimal <wasm-file>");
        return 1;
    }

    // Get argv[1] as a null-terminated path
    let path_ptr = unsafe { *argv.offset(1) };
    let path = unsafe {
        let len = libc::strlen(path_ptr as *const libc::c_char);
        core::slice::from_raw_parts(path_ptr, len + 1) // include null terminator for open()
    };

    let data = match read_file(path) {
        Some(d) => d,
        None => {
            eprintln!("Error reading wasm file");
            return 1;
        }
    };

    let imports = vec![];
    let mut instance = match Instance::new(&data, &imports) {
        Ok(inst) => inst,
        Err(err) => {
            eprintln!("Error instantiating module: {}", err);
            return 1;
        }
    };

    let result = instance.invoke("_start", &[]);
    let result = match result {
        Err(ref err) if err.to_string().contains("not found") => {
            instance.invoke("main", &[])
        }
        _ => result,
    };

    match result {
        Ok(_) => 0,
        Err(err) => {
            if let Some(code) = err.exit_code() {
                return code;
            }
            eprintln!("Error: {}", err);
            1
        }
    }
}
