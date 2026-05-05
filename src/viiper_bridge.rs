use std::sync::Arc;
use anyhow::Result;
use viiper_client::{AsyncViiperClient, AsyncDeviceStream};
use tokio::sync::mpsc;

pub struct ViiperManager {
    client: Arc<AsyncViiperClient>,
    bus_id: u32,
}

impl ViiperManager {
    pub async fn connect(addr: &str) -> Result<Self> {
        let client = Arc::new(AsyncViiperClient::new_with_password(addr.parse()?, String::new()));
        
        let buses = client.bus_list().await?;
        let bus_id = if let Some(&id) = buses.buses.first() {
            id
        } else {
            client.bus_create(None).await?.bus_id
        };

        Ok(Self { client, bus_id })
    }

    pub async fn ensure_bus(&mut self) -> Result<()> {
        let buses = self.client.bus_list().await?;
        if !buses.buses.contains(&self.bus_id) {
            self.bus_id = if let Some(&id) = buses.buses.first() {
                id
            } else {
                self.client.bus_create(None).await?.bus_id
            };
        }
        Ok(())
    }

    pub async fn create_virtual_xbox_controller(&mut self, name: &str) -> Result<(String, AsyncDeviceStream, mpsc::UnboundedReceiver<(u8, u8)>)> {
        self.ensure_bus().await?;
        
        let mut device_specific = std::collections::HashMap::new();
        device_specific.insert("name".to_string(), serde_json::Value::String(name.to_string()));
        
        let req = viiper_client::types::DeviceCreateRequest {
            r#type: Some("xbox360".to_string()),
            // Must remain None so Windows binds xusb22.sys (Xbox 360 driver).
            // Overriding VID/PID here causes the device to be unrecognized by games.
            id_vendor: None,
            id_product: None,
            device_specific: Some(device_specific),
        };
        
        let dev_info = self.client.bus_device_add(self.bus_id, &req).await?;
        let mut dev_stream = self.client.connect_device(self.bus_id, &dev_info.dev_id).await?;
        
        let (rumble_tx, rumble_rx) = mpsc::unbounded_channel();
        dev_stream.on_output(move |reader| {
            let rumble_tx = rumble_tx.clone();
            async move {
                let mut buf = [0u8; 2];
                let mut guard = reader.lock().await;
                if tokio::io::AsyncReadExt::read_exact(&mut *guard, &mut buf).await.is_ok() {
                    let _ = rumble_tx.send((buf[0], buf[1]));
                }
                Ok(())
            }
        })?;
        
        let dev_id = dev_info.dev_id.clone();
        
        Ok((dev_id, dev_stream, rumble_rx))
    }

    pub async fn remove_virtual_xbox_controller(&self, dev_id: &str) -> Result<()> {
        self.client.bus_device_remove(self.bus_id, Some(dev_id)).await?;
        Ok(())
    }
}
