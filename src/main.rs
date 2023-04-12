mod power_supply;

use std::io;
use std::os::fd::AsRawFd;

use mio::unix::SourceFd;
use mio::Events;
use mio::Interest;
use mio::Poll;
use mio::Token;
pub use power_supply::DeviceType;
pub use power_supply::PowerSupply;

fn main() -> io::Result<()>
{
    let mut power_supply = PowerSupply::new()?;

    let socket = udev::MonitorBuilder::new()?
        .match_subsystem("power_supply")?
        .listen()?;

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(5);

    poll.registry().register(
        &mut SourceFd(&socket.as_raw_fd()),
        Token(0),
        Interest::READABLE | Interest::WRITABLE,
    )?;

    loop {
        poll.poll(&mut events, None)?;
        events.clear();
        socket
            .iter()
            .for_each(|event| power_supply.set_device(event.device()));

        power_supply.set_charging_status();
        if power_supply.charging_status_changed() {
            println!(
                "Charging status changed: {}",
                power_supply.charging_status(),
            );
        }
    }
}
