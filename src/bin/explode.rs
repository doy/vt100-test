use std::io::Read as _;

#[derive(Default)]
struct Printer {
    chars: String,
}

impl Printer {
    fn append(&mut self, c: char) {
        self.chars.push(c);
    }

    fn flush(&mut self) {
        if !self.chars.is_empty() {
            println!("TEXT \"{}\"", self.chars);
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
                println!("ESC {}", b as char);
            }
            Some(i) => {
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
        match intermediates.get(0) {
            None => {
                println!("CSI {} {}", param_str(params), c);
            }
            Some(i) => {
                println!("CSI {} {} {}", *i as char, param_str(params), c);
            }
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]]) {
        self.flush();
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

fn osc_param_str(params: &[&[u8]]) -> String {
    let strs: Vec<_> = params
        .iter()
        .map(|b| format!("\"{}\"", std::string::String::from_utf8_lossy(*b)))
        .collect();
    strs.join(" ; ")
}

fn main() {
    env_logger::from_env(
        env_logger::Env::default().default_filter_or("error"),
    )
    .init();

    let mut ttyrec = ttyrec::Parser::new();
    let mut vte = vte::Parser::new();
    let mut printer = Printer::default();
    let mut file =
        std::fs::File::open(std::env::args().nth(1).unwrap()).unwrap();
    let mut frame_idx = 1;

    let mut buf = [0; 4096];
    loop {
        let n = file.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        ttyrec.add_bytes(&buf[..n]);
        while let Some(frame) = ttyrec.next_frame() {
            if frame_idx > 1 {
                println!();
            }
            println!("FRAME {}", frame_idx);
            frame_idx += 1;
            for b in frame.data {
                vte.advance(&mut printer, b);
            }
        }
    }
}
