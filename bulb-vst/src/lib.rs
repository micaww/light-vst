use bulb_core::{BulbConfig, BulbController};
use crossbeam_channel::{bounded, Receiver, Sender};
use nih_plug::prelude::*;
use std::sync::Arc;

enum BulbCommand {
    SetHue(u16)
}

pub struct BulbVst {
    params: Arc<BulbVstParams>,
    command_tx: Sender<BulbCommand>,
    _bulb_thread: Option<std::thread::JoinHandle<()>>,
}

#[derive(Params)]
struct BulbVstParams {
    #[id = "hue"]
    pub hue: FloatParam,
}

impl Default for BulbVst {
    fn default() -> Self {
        let (command_tx, command_rx) = bounded(100);

        // use separate thread for bulb comms, since vst must be real-time safe
        let bulb_thread = std::thread::spawn(move || {
            bulb_controller_thread(command_rx);
        });

        Self {
            params: Arc::new(BulbVstParams::default()),
            command_tx,
            _bulb_thread: Some(bulb_thread),
        }
    }
}

impl Default for BulbVstParams {
    fn default() -> Self {
        Self {
            hue: FloatParam::new(
                "Hue",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            )
            .with_unit(" °")
            .with_value_to_string(Arc::new(|value| {
                format!("{:.0}", value * 360.0)
            }))
            .with_string_to_value(Arc::new(|string| {
                string.trim_end_matches(" °")
                    .parse::<f32>()
                    .ok()
                    .map(|degrees| degrees / 360.0)
            })),
        }
    }
}

impl Plugin for BulbVst {
    const NAME: &'static str = "Bulb Controller";
    const VENDOR: &'static str = "Micah";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // we arent processing any audio, just midi signals
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: None,
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // process midi events
        while let Some(event) = context.next_event() {
            match event {
                NoteEvent::MidiCC {
                    timing: _,
                    channel: _,
                    cc,
                    value,
                } => {
                    // mod wheel
                    if cc == 1 {
                        let midi_value = (value * 127.0) as u8;
                        let hue = bulb_core::midi_to_hue(midi_value);

                        let _ = self.command_tx.try_send(BulbCommand::SetHue(hue));
                    }
                }
                _ => {}
            }
        }

        ProcessStatus::Normal
    }
}

fn bulb_controller_thread(command_rx: Receiver<BulbCommand>) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let mut controller: Option<BulbController> = None;
        let mut last_hue: Option<u16> = None;

        let default_config = BulbConfig::new(
            "eb052a1de220394996xwke",
            "!BY}~:dab1nuT;'n",
            "192.168.0.124",
            "3.3",
        );

        match BulbController::new(default_config) {
            Ok(mut ctrl) => {
                if ctrl.connect().await.is_ok() {
                    // initialize to green
                    let _ = ctrl.set_color(120, 1000, 1000).await;
                    controller = Some(ctrl);
                    nih_log!("Bulb connected successfully");
                } else {
                    nih_error!("Failed to connect to bulb");
                }
            }
            Err(e) => {
                nih_error!("Failed to create bulb controller: {}", e);
            }
        }

        while let Ok(command) = command_rx.recv() {
            match command {
                BulbCommand::SetHue(hue) => {
                    if last_hue != Some(hue) {
                        if let Some(ref mut ctrl) = controller {
                            match ctrl.set_color(hue, 1000, 1000).await {
                                Ok(_) => {
                                    last_hue = Some(hue);
                                    nih_trace!("Set bulb hue to {}", hue);
                                }
                                Err(e) => {
                                    nih_error!("Failed to set bulb color: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

impl Vst3Plugin for BulbVst {
    const VST3_CLASS_ID: [u8; 16] = *b"BulbVstMicahfart";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Tools,
    ];
}

nih_export_vst3!(BulbVst);
