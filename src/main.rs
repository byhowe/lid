use std::io;
use std::os::fd::AsRawFd;

use mio::unix::SourceFd;
use mio::Events;
use mio::Interest;
use mio::Poll;
use mio::Token;

fn main() -> io::Result<()>
{
    let socket = udev::MonitorBuilder::new()?
        .match_subsystem("power_supply")?
        .listen()?;

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(4);

    poll.registry().register(
        &mut SourceFd(&socket.as_raw_fd()),
        Token(0),
        Interest::READABLE | Interest::WRITABLE,
    )?;

    loop {
        poll.poll(&mut events, None)?;

        for event in &events {
            if event.token() == Token(0) && event.is_writable() {
                socket.iter().for_each(print_event);
            }
        }
    }
}

fn print_event(event: udev::Event)
{
    println!(
        "{}: {} {} (subsystem={}, sysname={}, devtype={})",
        event.sequence_number(),
        event.event_type(),
        event.syspath().to_str().unwrap_or("---"),
        event
            .subsystem()
            .map_or("", |s| { s.to_str().unwrap_or("") }),
        event.sysname().to_str().unwrap_or(""),
        event.devtype().map_or("", |s| { s.to_str().unwrap_or("") })
    );
}
