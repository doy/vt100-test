use std::io::{Read as _, Write as _};
use unicode_width::UnicodeWidthStr as _;

struct Printer {
    base: std::time::Instant,
    offset: std::time::Duration,
    chars: String,
    writer: ttyrec::Creator,
    frames: Vec<ttyrec::Frame>,
}

impl Printer {
    fn new() -> Self {
        Self {
            base: std::time::Instant::now(),
            offset: std::time::Duration::default(),
            chars: String::default(),
            writer: ttyrec::Creator::default(),
            frames: vec![],
        }
    }

    fn append(&mut self, c: char) {
        self.chars.push(c);
    }

    fn frame(&mut self, bytes: &[u8]) {
        self.frames
            .push(self.writer.frame_at(self.base + self.offset, bytes));
    }

    fn flush(&mut self) {
        if !self.chars.is_empty() {
            self.frame(self.chars.clone().as_bytes());
            println!("TEXT({}) \"{}\"", self.chars.width(), self.chars);
            self.chars.clear();
        }
    }
}

impl vte::Perform for Printer {
    fn print(&mut self, c: char) {
        self.append(c);
    }

    fn execute(&mut self, b: u8) {
        self.flush();
        self.frame(&[b]);
        println!("CTRL {}", (b + b'@') as char);
    }

    fn esc_dispatch(
        &mut self,
        _params: &[i64],
        intermediates: &[u8],
        _ignore: bool,
        b: u8,
    ) {
        self.flush();
        match intermediates.get(0) {
            None => {
                self.frame(&[0x1b, b]);
                println!("ESC {}", b as char);
            }
            Some(i) => {
                self.frame(&[0x1b, *i, b]);
                println!("ESC {} {}", *i as char, b as char);
            }
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &[i64],
        intermediates: &[u8],
        _ignore: bool,
        c: char,
    ) {
        self.flush();
        let mut bytes = vec![0x1b, b'['];
        match intermediates.get(0) {
            None => {
                bytes.extend(param_bytes(params));
                bytes.push(c as u8);
                self.frame(&bytes);
                println!("CSI {} {}", param_str(params), c);
            }
            Some(i) => {
                bytes.push(*i);
                bytes.extend(param_bytes(params));
                bytes.push(c as u8);
                self.frame(&bytes);
                println!("CSI {} {} {}", *i as char, param_str(params), c);
            }
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]]) {
        self.flush();
        let mut bytes = vec![0x1b, b']'];
        bytes.extend(osc_param_bytes(params));
        bytes.push(7);
        self.frame(&bytes);
        println!("OSC {}", osc_param_str(params));
    }

    fn hook(&mut self, params: &[i64], intermediates: &[u8], _ignore: bool) {
        self.flush();
        match intermediates.get(0) {
            None => {
                println!("DCS {}", param_str(params));
            }
            Some(i) => {
                println!("DCS {} {}", *i as char, param_str(params));
            }
        }
    }
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}
}

fn param_str(params: &[i64]) -> String {
    let strs: Vec<_> = params
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    strs.join(" ; ")
}

fn param_bytes(params: &[i64]) -> Vec<u8> {
    let strs: Vec<_> = params
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    strs.join(";").into_bytes()
}

fn osc_param_str(params: &[&[u8]]) -> String {
    let strs: Vec<_> = params
        .iter()
        .map(|b| format!("\"{}\"", std::string::String::from_utf8_lossy(*b)))
        .collect();
    strs.join(" ; ")
}

fn osc_param_bytes(params: &[&[u8]]) -> Vec<u8> {
    let strs: Vec<_> = params
        .iter()
        .map(|b| format!("\"{}\"", std::string::String::from_utf8_lossy(*b)))
        .collect();
    strs.join(";").into_bytes()
}

fn main() {
    env_logger::from_env(
        env_logger::Env::default().default_filter_or("error"),
    )
    .init();

    let filename = std::env::args().nth(1).unwrap();
    let mut reader = ttyrec::Parser::new();
    let mut vte = vte::Parser::new();
    let mut printer = Printer::new();
    let mut file = std::fs::File::open(filename.clone()).unwrap();
    let mut frame_idx = 1;

    let mut buf = [0; 4096];
    loop {
        let n = file.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        reader.add_bytes(&buf[..n]);
        while let Some(frame) = reader.next_frame() {
            if frame_idx > 1 {
                println!();
            }
            println!("FRAME {}", frame_idx);
            frame_idx += 1;
            printer.offset = frame.time;
            for b in frame.data {
                vte.advance(&mut printer, b);
            }
            printer.flush();
        }
    }

    let mut file =
        std::fs::File::create(format!("exploded-{}", filename)).unwrap();
    for frame in printer.frames {
        let data: Vec<_> = std::convert::TryFrom::try_from(frame).unwrap();
        file.write_all(&data).unwrap();
    }
}
