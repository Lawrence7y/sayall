use windows::Devices::Bluetooth::BluetoothAdapter;
use windows::Devices::WiFiDirect::WiFiDirectAdvertisementPublisher;

fn main() {
    println!("== BluetoothAdapter ==");
    match BluetoothAdapter::GetDefaultAsync() {
        Ok(op) => match op.get() {
            Ok(adapter) => {
                println!("bluetooth_address=0x{:012X}", adapter.BluetoothAddress().unwrap_or(0));
                println!("is_low_energy_supported={:?}", adapter.IsLowEnergySupported());
                println!("is_peripheral_role_supported={:?}", adapter.IsPeripheralRoleSupported());
                println!("is_central_role_supported={:?}", adapter.IsCentralRoleSupported());
                println!("is_advertisement_offload_supported={:?}", adapter.IsAdvertisementOffloadSupported());
            }
            Err(e) => println!("GetDefaultAsync await failed: {e}"),
        },
        Err(e) => println!("GetDefaultAsync call failed: {e}"),
    }
    println!("== WiFiDirectAdvertisementPublisher ==");
    match WiFiDirectAdvertisementPublisher::new() {
        Ok(publisher) => {
            println!("publisher_created=true");
            match publisher.Start() {
                Ok(()) => {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    println!("start=ok status={:?}", publisher.Status());
                    if let Ok(listener) = publisher.Advertisement() {
                        if let Ok(autonomous) = listener.IsAutonomousGroupOwnerEnabled() {
                            println!("is_autonomous_group={autonomous}");
                        }
                        if let Ok(listen_state) = listener.ListenStateDiscoverability() {
                            println!("listen_state_discoverable={listen_state}");
                        }
                    }
                    let _ = publisher.Stop();
                }
                Err(e) => println!("start failed: {e}"),
            }
        }
        Err(e) => println!("create failed: {e}"),
    }
}

