//! The narrow BlueZ boundary for the Linux client.
//!
//! It deliberately knows nothing about Tauri or the browser bridge, so the
//! Bluetooth behaviour can be tested without WebKitGTK or a desktop session.

use std::{fmt, pin::Pin};

use async_trait::async_trait;
use bluer::{
    gatt::{
        remote::{Characteristic, CharacteristicWriteRequest},
        WriteOp,
    },
    Adapter, AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport, Session,
};
use futures::{stream::Stream, StreamExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterInfo {
    pub name: String,
    pub powered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterState {
    Unavailable,
    Disabled { name: String },
    Ready { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: Option<String>,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GattService {
    pub uuid: String,
    pub characteristics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacteristicId {
    pub device_id: String,
    pub service_uuid: String,
    pub characteristic_uuid: String,
}

pub type NotificationStream = Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>;
pub type DeviceStream = Pin<Box<dyn Stream<Item = Result<DeviceInfo, Error>> + Send>>;

pub fn adapter_state(adapter: Option<AdapterInfo>) -> AdapterState {
    match adapter {
        None => AdapterState::Unavailable,
        Some(AdapterInfo {
            name,
            powered: false,
        }) => AdapterState::Disabled { name },
        Some(AdapterInfo {
            name,
            powered: true,
        }) => AdapterState::Ready { name },
    }
}

#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

#[async_trait]
pub trait BluetoothAdapter: Send + Sync {
    async fn state(&self) -> Result<AdapterState, Error>;
    async fn devices(&self) -> Result<Vec<DeviceInfo>, Error>;
    async fn discover(&self, services: &[String]) -> Result<DeviceStream, Error>;
    async fn connect(&self, device_id: &str) -> Result<(), Error>;
    async fn disconnect(&self, device_id: &str) -> Result<(), Error>;
    async fn services(&self, device_id: &str) -> Result<Vec<GattService>, Error>;
    async fn write(
        &self,
        characteristic: &CharacteristicId,
        value: &[u8],
        with_response: bool,
    ) -> Result<(), Error>;
    async fn notifications(
        &self,
        characteristic: &CharacteristicId,
    ) -> Result<NotificationStream, Error>;
}

/// A connection to the default BlueZ adapter.
pub struct BluezAdapter {
    _session: Session,
    adapter: Adapter,
}

impl BluezAdapter {
    /// Opens the system D-Bus connection and selects BlueZ's default adapter.
    pub async fn open() -> Result<Self, Error> {
        let session = Session::new()
            .await
            .map_err(|error| Error(error.to_string()))?;
        let adapter = session
            .default_adapter()
            .await
            .map_err(|error| Error(error.to_string()))?;
        Ok(Self {
            _session: session,
            adapter,
        })
    }

    /// Reports whether the selected adapter can be used without changing its state.
    async fn adapter_state(&self) -> Result<AdapterState, Error> {
        let info = AdapterInfo {
            name: self.adapter.name().to_owned(),
            powered: self
                .adapter
                .is_powered()
                .await
                .map_err(|error| Error(error.to_string()))?,
        };
        Ok(adapter_state(Some(info)))
    }

    fn device(&self, device_id: &str) -> Result<bluer::Device, Error> {
        let address = device_id
            .parse::<Address>()
            .map_err(|error| Error(error.to_string()))?;
        self.adapter
            .device(address)
            .map_err(|error| Error(error.to_string()))
    }

    async fn characteristic(&self, id: &CharacteristicId) -> Result<Characteristic, Error> {
        let device = self.device(&id.device_id)?;
        for service in device
            .services()
            .await
            .map_err(|error| Error(error.to_string()))?
        {
            if !matches_uuid(
                &service
                    .uuid()
                    .await
                    .map_err(|error| Error(error.to_string()))?,
                &id.service_uuid,
            ) {
                continue;
            }
            for characteristic in service
                .characteristics()
                .await
                .map_err(|error| Error(error.to_string()))?
            {
                if matches_uuid(
                    &characteristic
                        .uuid()
                        .await
                        .map_err(|error| Error(error.to_string()))?,
                    &id.characteristic_uuid,
                ) {
                    return Ok(characteristic);
                }
            }
        }
        Err(Error("Bluetooth characteristic was not found.".into()))
    }
}

fn matches_uuid(uuid: &impl fmt::Display, wanted: &str) -> bool {
    uuid.to_string().eq_ignore_ascii_case(wanted)
}

#[async_trait]
impl BluetoothAdapter for BluezAdapter {
    async fn state(&self) -> Result<AdapterState, Error> {
        self.adapter_state().await
    }

    async fn devices(&self) -> Result<Vec<DeviceInfo>, Error> {
        let addresses = self
            .adapter
            .device_addresses()
            .await
            .map_err(|error| Error(error.to_string()))?;
        let mut devices = Vec::with_capacity(addresses.len());
        for address in addresses {
            let device = self
                .adapter
                .device(address)
                .map_err(|error| Error(error.to_string()))?;
            devices.push(DeviceInfo {
                id: address.to_string(),
                name: device
                    .name()
                    .await
                    .map_err(|error| Error(error.to_string()))?,
                services: device
                    .uuids()
                    .await
                    .map_err(|error| Error(error.to_string()))?
                    .unwrap_or_default()
                    .into_iter()
                    .map(|uuid| uuid.to_string())
                    .collect(),
            });
        }
        Ok(devices)
    }

    async fn discover(&self, services: &[String]) -> Result<DeviceStream, Error> {
        let uuids = services
            .iter()
            .map(|uuid| {
                uuid.parse::<bluer::Uuid>()
                    .map_err(|error| Error(error.to_string()))
            })
            .collect::<Result<_, _>>()?;
        self.adapter
            .set_discovery_filter(DiscoveryFilter {
                uuids,
                transport: DiscoveryTransport::Le,
                ..Default::default()
            })
            .await
            .map_err(|error| Error(error.to_string()))?;
        let adapter = self.adapter.clone();
        let events = self
            .adapter
            .discover_devices()
            .await
            .map_err(|error| Error(error.to_string()))?;
        Ok(events
            .filter_map(move |event| {
                let adapter = adapter.clone();
                async move {
                    let AdapterEvent::DeviceAdded(address) = event else {
                        return None;
                    };
                    let device = match adapter.device(address) {
                        Ok(device) => device,
                        Err(error) => return Some(Err(Error(error.to_string()))),
                    };
                    let name = match device.name().await {
                        Ok(name) => name,
                        Err(error) => return Some(Err(Error(error.to_string()))),
                    };
                    let services = match device.uuids().await {
                        Ok(uuids) => uuids
                            .unwrap_or_default()
                            .into_iter()
                            .map(|uuid| uuid.to_string())
                            .collect(),
                        Err(error) => return Some(Err(Error(error.to_string()))),
                    };
                    Some(Ok(DeviceInfo {
                        id: address.to_string(),
                        name,
                        services,
                    }))
                }
            })
            .boxed())
    }

    async fn connect(&self, device_id: &str) -> Result<(), Error> {
        self.device(device_id)?
            .connect()
            .await
            .map_err(|error| Error(error.to_string()))
    }

    async fn disconnect(&self, device_id: &str) -> Result<(), Error> {
        self.device(device_id)?
            .disconnect()
            .await
            .map_err(|error| Error(error.to_string()))
    }

    async fn services(&self, device_id: &str) -> Result<Vec<GattService>, Error> {
        let device = self.device(device_id)?;
        let mut services = Vec::new();
        for service in device
            .services()
            .await
            .map_err(|error| Error(error.to_string()))?
        {
            let mut characteristics = Vec::new();
            for characteristic in service
                .characteristics()
                .await
                .map_err(|error| Error(error.to_string()))?
            {
                characteristics.push(
                    characteristic
                        .uuid()
                        .await
                        .map_err(|error| Error(error.to_string()))?
                        .to_string(),
                );
            }
            services.push(GattService {
                uuid: service
                    .uuid()
                    .await
                    .map_err(|error| Error(error.to_string()))?
                    .to_string(),
                characteristics,
            });
        }
        Ok(services)
    }

    async fn write(
        &self,
        id: &CharacteristicId,
        value: &[u8],
        with_response: bool,
    ) -> Result<(), Error> {
        let request = CharacteristicWriteRequest {
            op_type: if with_response {
                WriteOp::Request
            } else {
                WriteOp::Command
            },
            ..Default::default()
        };
        self.characteristic(id)
            .await?
            .write_ext(value, &request)
            .await
            .map_err(|error| Error(error.to_string()))
    }

    async fn notifications(&self, id: &CharacteristicId) -> Result<NotificationStream, Error> {
        Ok(self
            .characteristic(id)
            .await?
            .notify()
            .await
            .map_err(|error| Error(error.to_string()))?
            .boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        adapter_state, AdapterInfo, AdapterState, BluetoothAdapter, CharacteristicId, DeviceInfo,
        DeviceStream, Error, GattService, NotificationStream,
    };
    use futures::StreamExt;

    struct FakeAdapter(AdapterState);

    #[async_trait::async_trait]
    impl BluetoothAdapter for FakeAdapter {
        async fn state(&self) -> Result<AdapterState, Error> {
            Ok(self.0.clone())
        }

        async fn devices(&self) -> Result<Vec<DeviceInfo>, Error> {
            Ok(vec![DeviceInfo {
                id: "AA:BB:CC:DD:EE:FF".into(),
                name: Some("Trainer".into()),
                services: vec![],
            }])
        }

        async fn discover(&self, _services: &[String]) -> Result<DeviceStream, Error> {
            Ok(Box::pin(futures::stream::iter(vec![Ok(DeviceInfo {
                id: "AA:BB:CC:DD:EE:FF".into(),
                name: Some("Trainer".into()),
                services: vec![],
            })])))
        }

        async fn connect(&self, _device_id: &str) -> Result<(), Error> {
            Ok(())
        }

        async fn disconnect(&self, _device_id: &str) -> Result<(), Error> {
            Ok(())
        }

        async fn services(&self, _device_id: &str) -> Result<Vec<GattService>, Error> {
            Ok(vec![GattService {
                uuid: "00001826-0000-1000-8000-00805f9b34fb".into(),
                characteristics: vec!["00002ad9-0000-1000-8000-00805f9b34fb".into()],
            }])
        }

        async fn write(
            &self,
            _characteristic: &CharacteristicId,
            _value: &[u8],
            _with_response: bool,
        ) -> Result<(), Error> {
            Ok(())
        }

        async fn notifications(
            &self,
            _characteristic: &CharacteristicId,
        ) -> Result<NotificationStream, Error> {
            Ok(Box::pin(futures::stream::empty()))
        }
    }

    #[tokio::test]
    async fn adapter_state_is_available_through_the_adapter_interface() {
        let adapter = FakeAdapter(AdapterState::Ready {
            name: "hci0".into(),
        });

        assert_eq!(
            adapter.state().await.unwrap(),
            AdapterState::Ready {
                name: "hci0".into()
            }
        );
    }

    #[tokio::test]
    async fn gatt_operations_are_available_through_the_adapter_interface() {
        let adapter = FakeAdapter(AdapterState::Ready {
            name: "hci0".into(),
        });
        let device = adapter
            .discover(&["00001826-0000-1000-8000-00805f9b34fb".into()])
            .await
            .unwrap()
            .next()
            .await
            .unwrap()
            .unwrap();
        let service = adapter.services(&device.id).await.unwrap().pop().unwrap();
        let characteristic = CharacteristicId {
            device_id: device.id,
            service_uuid: service.uuid,
            characteristic_uuid: service.characteristics.into_iter().next().unwrap(),
        };

        adapter.connect(&characteristic.device_id).await.unwrap();
        adapter.write(&characteristic, &[1, 2], true).await.unwrap();
        assert!(adapter
            .notifications(&characteristic)
            .await
            .unwrap()
            .next()
            .await
            .is_none());
        adapter.disconnect(&characteristic.device_id).await.unwrap();
    }

    #[test]
    fn no_adapter_is_unavailable() {
        assert_eq!(adapter_state(None), AdapterState::Unavailable);
    }

    #[test]
    fn powered_adapter_is_ready() {
        let adapter = AdapterInfo {
            name: "hci0".into(),
            powered: true,
        };

        assert_eq!(
            adapter_state(Some(adapter)),
            AdapterState::Ready {
                name: "hci0".into()
            }
        );
    }

    #[test]
    fn unpowered_adapter_needs_enabling() {
        let adapter = AdapterInfo {
            name: "hci1".into(),
            powered: false,
        };

        assert_eq!(
            adapter_state(Some(adapter)),
            AdapterState::Disabled {
                name: "hci1".into()
            }
        );
    }
}
