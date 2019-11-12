use futures::stream::Stream as _;
use std::io::Write as _;

fn main() {
    env_logger::from_env(
        env_logger::Env::default().default_filter_or("error"),
    )
    .init();
    let (cmd, args) = if std::env::args().count() > 1 {
        (
            std::env::args().nth(1).unwrap(),
            std::env::args().skip(2).collect(),
        )
    } else {
        (
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
            vec![],
        )
    };
    tokio::run(Passthrough::new(&cmd, &args));
}

struct Passthrough {
    term: vt100::Parser,
    process: tokio_pty_process_stream::ResizingProcess<Stdin>,
    raw_screen: Option<crossterm::screen::RawScreen>,
    alternate_screen: Option<crossterm::screen::AlternateScreen>,
    done: bool,
}

impl Passthrough {
    fn new(cmd: &str, args: &[String]) -> Self {
        let input = Stdin::new();
        let process = tokio_pty_process_stream::ResizingProcess::new(
            tokio_pty_process_stream::Process::new(cmd, args, input),
        );
        Self {
            term: vt100::Parser::default(),
            process,
            raw_screen: None,
            alternate_screen: None,
            done: false,
        }
    }

    fn err<T: std::fmt::Display>(&mut self, e: T) {
        self.alternate_screen = None;
        self.raw_screen = None;
        eprintln!("{}", e);
    }
}

impl futures::future::Future for Passthrough {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            let event = futures::try_ready!(self
                .process
                .poll()
                .map_err(|e| self.err(e)));
            match event {
                Some(tokio_pty_process_stream::Event::CommandStart {
                    ..
                }) => {
                    if self.raw_screen.is_none() {
                        self.raw_screen = Some(
                            crossterm::screen::RawScreen::into_raw_mode()
                                .map_err(|e| self.err(e))?,
                        );
                    }
                    if self.alternate_screen.is_none() {
                        self.alternate_screen = Some(
                            crossterm::screen::AlternateScreen::to_alternate(
                                false,
                            )
                            .map_err(|e| self.err(e))?,
                        );
                    }
                    write(&self.term.screen().contents_formatted())
                        .map_err(|e| self.err(e))?;
                }
                Some(tokio_pty_process_stream::Event::CommandExit {
                    ..
                }) => {
                    self.done = true;
                }
                Some(tokio_pty_process_stream::Event::Output { data }) => {
                    let screen = self.term.screen().clone();
                    self.term.process(&data);
                    write(&self.term.screen().contents_diff(&screen))
                        .map_err(|e| self.err(e))?;
                }
                Some(tokio_pty_process_stream::Event::Resize {
                    size: (rows, cols),
                }) => {
                    self.term.set_size(rows, cols);
                }
                None => {
                    if !self.done {
                        unreachable!()
                    }
                    return Ok(futures::Async::Ready(()));
                }
            }
        }
    }
}

fn write(data: &[u8]) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(data)?;
    stdout.flush()?;
    Ok(())
}

struct EventedStdin;

const STDIN: i32 = 0;

impl std::io::Read for EventedStdin {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();
        stdin.read(buf)
    }
}

impl mio::Evented for EventedStdin {
    fn register(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> std::io::Result<()> {
        let fd = STDIN as std::os::unix::io::RawFd;
        let eventedfd = mio::unix::EventedFd(&fd);
        eventedfd.register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> std::io::Result<()> {
        let fd = STDIN as std::os::unix::io::RawFd;
        let eventedfd = mio::unix::EventedFd(&fd);
        eventedfd.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> std::io::Result<()> {
        let fd = STDIN as std::os::unix::io::RawFd;
        let eventedfd = mio::unix::EventedFd(&fd);
        eventedfd.deregister(poll)
    }
}

pub struct Stdin {
    input: tokio::reactor::PollEvented2<EventedStdin>,
}

impl Stdin {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for Stdin {
    fn default() -> Self {
        Self {
            input: tokio::reactor::PollEvented2::new(EventedStdin),
        }
    }
}

impl std::io::Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.input.read(buf)
    }
}

impl tokio::io::AsyncRead for Stdin {
    fn poll_read(
        &mut self,
        buf: &mut [u8],
    ) -> std::result::Result<futures::Async<usize>, tokio::io::Error> {
        // XXX this is why i had to do the EventedFd thing - poll_read on its
        // own will block reading from stdin, so i need a way to explicitly
        // check readiness before doing the read
        let ready = mio::Ready::readable();
        match self.input.poll_read_ready(ready)? {
            futures::Async::Ready(_) => {
                let res = self.input.poll_read(buf);

                // XXX i'm pretty sure this is wrong (if the single poll_read
                // call didn't return all waiting data, clearing read ready
                // state means that we won't get the rest until some more data
                // beyond that appears), but i don't know that there's a way
                // to do it correctly given that poll_read blocks
                self.input.clear_read_ready(ready)?;

                res
            }
            futures::Async::NotReady => Ok(futures::Async::NotReady),
        }
    }
}
