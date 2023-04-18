use std::ffi::OsStr;
use std::fmt::Display;
use std::io;
use std::os::fd::AsRawFd;

use mio::event::Source;
use mio::unix::SourceFd;
use udev::MonitorSocket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType
{
    /// Battery device.
    Battery,
    /// AC adapter (i.e., the power supply) of a device.
    Adapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status
{
    Discharging,
    Charging,
    Unknown,
}

impl Status
{
    fn read_from_battery_device(dev: &udev::Device) -> Self
    {
        let status = dev
            .property_value("POWER_SUPPLY_STATUS")
            .map(OsStr::to_str)
            .unwrap_or_default()
            .unwrap_or_default();
        match status {
            "Charging" => Self::Charging,
            "Discharging" | "Not charging" => Self::Discharging,
            _ => Self::Unknown,
        }
    }

    fn read_from_adapter_device(dev: &udev::Device) -> Self
    {
        let online = dev
            .property_value("POWER_SUPPLY_ONLINE")
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .parse::<i32>()
            .unwrap_or(-1);
        match online {
            0 => Status::Discharging,
            1 | 2 => Status::Charging,
            _ => Status::Unknown,
        }
    }
}

impl Display for Status
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        match *self {
            Status::Discharging => write!(f, "Discharging"),
            Status::Charging => write!(f, "Charging"),
            Status::Unknown => write!(f, "Unknown"),
        }
    }
}

pub struct PowerSupply
{
    socket: Option<udev::MonitorSocket>,

    bat: Option<udev::Device>,
    adp: Option<udev::Device>,

    status: Status,
    status_changed: bool,
}

impl Source for PowerSupply
{
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> io::Result<()>
    {
        SourceFd(&self.monitor_socket()?.as_raw_fd()).register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> io::Result<()>
    {
        SourceFd(&self.monitor_socket()?.as_raw_fd()).reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> io::Result<()>
    {
        SourceFd(&self.monitor_socket()?.as_raw_fd()).deregister(registry)
    }
}

impl PowerSupply
{
    #[must_use]
    pub fn new() -> Self
    {
        Self {
            socket: None,
            bat: None,
            adp: None,
            status: Status::Unknown,
            status_changed: true,
        }
    }

    pub fn update(&mut self) -> io::Result<()>
    {
        self.monitor_socket()?
            .iter()
            .for_each(|event| self.set_device(event.device()));
        self.current_charging_status()?;
        Ok(())
    }

    #[must_use]
    pub fn charging_status_changed(&self) -> bool
    {
        self.status_changed
    }

    /// Returns the last charging status detected by the udev driver.
    #[must_use]
    pub fn charging_status(&self) -> Status
    {
        self.status
    }

    fn enumerate(&mut self) -> io::Result<()>
    {
        let mut enumerator = udev::Enumerator::new()?;
        enumerator.match_subsystem("power_supply")?;
        let devices = enumerator.scan_devices()?.collect::<Vec<_>>();

        assert!(
            devices.len() == 2,
            "Failed to find two power supply devices!"
        );

        devices.into_iter().for_each(|dev| self.set_device(dev));

        Ok(())
    }

    fn monitor_socket(&mut self) -> io::Result<&MonitorSocket>
    {
        if self.socket.is_some() {
            Ok(unsafe { self.socket.as_ref().unwrap_unchecked() })
        } else {
            self.socket = Some(
                udev::MonitorBuilder::new()?
                    .match_subsystem("power_supply")?
                    .listen()?,
            );
            Ok(unsafe { self.socket.as_ref().unwrap_unchecked() })
        }
    }

    fn set_device(&mut self, dev: udev::Device)
    {
        match Self::device_type(&dev) {
            Some(DeviceType::Battery) => self.bat = Some(dev),
            Some(DeviceType::Adapter) => self.adp = Some(dev),
            None => todo!(),
        }
    }

    fn set_devices_if_not_set(&mut self) -> io::Result<()>
    {
        if self.bat.is_none() || self.adp.is_none() {
            self.enumerate()?;
        }
        Ok(())
    }

    #[must_use]
    fn device_type(dev: &udev::Device) -> Option<DeviceType>
    {
        match dev.parent()?.driver()?.to_str()? {
            "battery" => Some(DeviceType::Battery),
            "ac" => Some(DeviceType::Adapter),
            _ => {
                let sysname = dev.sysname().to_string_lossy();
                if sysname.starts_with("BAT") {
                    Some(DeviceType::Battery)
                } else if sysname.starts_with("ADP") {
                    Some(DeviceType::Adapter)
                } else {
                    None
                }
            }
        }
    }

    /// Fetch the current status and set the last charging status to the current
    /// one.
    fn current_charging_status(&mut self) -> io::Result<()>
    {
        self.set_devices_if_not_set()?;
        let status =
            match Status::read_from_adapter_device(unsafe { self.adp.as_ref().unwrap_unchecked() })
            {
                Status::Unknown => Status::read_from_battery_device(unsafe {
                    self.bat.as_ref().unwrap_unchecked()
                }),
                status => status,
            };
        self.status_changed = status != self.status;
        self.status = status;
        Ok(())
    }
}

impl Default for PowerSupply
{
    fn default() -> Self
    {
        Self::new()
    }
}
