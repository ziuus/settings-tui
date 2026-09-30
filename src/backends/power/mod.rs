use super::{BatteryState, PowerBackend, PowerInfo};
use crate::dbus::power::{PowerProfilesProxy, UPowerDeviceProxy, UPowerProxy};
use anyhow::Result;
use async_trait::async_trait;
use zbus::Connection;

pub struct UPowerBackend {
    connection: Connection,
}

impl UPowerBackend {
    pub async fn new() -> Result<Self> {
        let connection = Connection::system().await?;
        Ok(Self { connection })
    }
}

#[async_trait]
impl PowerBackend for UPowerBackend {
    async fn get_info(&self) -> Result<PowerInfo> {
        let upower = UPowerProxy::new(&self.connection).await?;
        let on_battery = upower.on_battery().await.unwrap_or(false);

        let mut battery_percentage = 100.0;
        let mut battery_state = BatteryState::Unknown;

        if let Ok(device) = UPowerDeviceProxy::new(&self.connection).await {
            battery_percentage = device.percentage().await.unwrap_or(100.0);

            let state_val = device.state().await.unwrap_or(0);
            battery_state = match state_val {
                1 => BatteryState::Charging,
                2 => BatteryState::Discharging,
                3 => BatteryState::Empty,
                4 => BatteryState::FullyCharged,
                5 => BatteryState::PendingCharge,
                6 => BatteryState::PendingDischarge,
                _ => BatteryState::Unknown,
            };
        }

        let mut power_profile = None;
        if let Ok(profiles) = PowerProfilesProxy::new(&self.connection).await {
            if let Ok(profile) = profiles.active_profile().await {
                power_profile = Some(profile);
            }
        }

        Ok(PowerInfo {
            on_battery,
            battery_percentage,
            battery_state,
            power_profile,
        })
    }

    async fn set_power_profile(&self, profile: &str) -> Result<()> {
        let profiles = PowerProfilesProxy::new(&self.connection).await?;
        profiles.set_active_profile(profile).await?;
        Ok(())
    }
}
