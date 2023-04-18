mod power_supply;

use std::io;

use mio::event::Source;
use mio::Events;
use mio::Interest;
use mio::Poll;
use mio::Token;
pub use power_supply::DeviceType;
pub use power_supply::PowerSupply;

fn main() -> io::Result<()>
{
    let mut power_supply = PowerSupply::new();

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(1024);

    power_supply.register(
        poll.registry(),
        Token(0),
        Interest::READABLE | Interest::WRITABLE,
    )?;

    loop {
        poll.poll(&mut events, None)?;
        events.clear();
        power_supply.update()?;

        if power_supply.charging_status_changed() {
            println!(
                "Charging status changed: {}",
                power_supply.charging_status(),
            );
        }
    }
}
