use anyhow::{anyhow, Result};
use bulb_core::{BulbConfig, BulbController, midi_to_hue};
use midir::{Ignore, MidiInput, MidiInputConnection};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    let config = BulbConfig::new(
        "eb052a1de220394996xwke",
        "!BY}~:dab1nuT;'n",
        "192.168.0.124",
        "3.3",
    );

    let mut controller = BulbController::new(config)?;
    println!("Connecting to device...");
    controller.connect().await?;
    println!("Connected!");

    controller.set_color(120, 1000, 1000).await?;
    println!("Bulb initialized to green");

    let (tx, mut rx) = mpsc::unbounded_channel::<u16>();

    let _connection = start_midi_listener(tx)?;
    println!("MIDI listener started");

    tokio::spawn(async move {
        while let Some(hue) = rx.recv().await {
            if let Err(e) = controller.set_color(hue, 1000, 1000, true).await {
                eprintln!("Error setting bulb color: {}", e);
            }
        }
    });

    // keep the connection alive
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

fn start_midi_listener(tx: mpsc::UnboundedSender<u16>) -> Result<MidiInputConnection<()>> {
    let mut midi_in = MidiInput::new("bulb-controller")?;
    midi_in.ignore(Ignore::None);

    let in_ports = midi_in.ports();
    if in_ports.is_empty() {
        return Err(anyhow!("No MIDI input ports available."));
    }

    let in_port = in_ports.get(0).unwrap();
    let port_name = midi_in.port_name(in_port)?;
    println!("Connecting to MIDI port: {}", port_name);

    let connection = midi_in.connect(
        in_port,
        "bulb-controller-input",
        move |_, message, _| {
            match message {
                [0xB0, 0x01, value] => {
                    let hue = midi_to_hue(*value);
                    println!("Mod Wheel: {} -> Hue: {}", value, hue);

                    if let Err(e) = tx.send(hue) {
                        eprintln!("Error sending hue to channel: {}", e);
                    }
                }
                _ => {}
            }
        },
        (),
    )?;

    Ok(connection)
}
