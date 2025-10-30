use bulb_core::{BulbConfig, BulbController};
use crossbeam_channel::{bounded, Receiver, Sender};
use nih_plug::prelude::*;
use std::sync::Arc;

enum BulbCommand {
    SetHSV(u16, u16, u16, bool),
}

pub struct BulbVst {
    params: Arc<BulbVstParams>,
    command_tx: Sender<BulbCommand>,
    _bulb_thread: Option<std::thread::JoinHandle<()>>,
    last_hue: u16,
    last_saturation: u16,
    last_brightness: u16,
}

#[derive(Params)]
struct BulbVstParams {
    #[id = "hue"]
    pub hue: FloatParam,
    #[id = "saturation"]
    pub saturation: FloatParam,
    #[id = "brightness"]
    pub brightness: FloatParam,
    #[id = "immediate"]
    pub immediate: BoolParam,
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
            last_hue: u16::MAX,
            last_saturation: u16::MAX,
            last_brightness: u16::MAX,
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
            saturation: FloatParam::new(
                "Saturation",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            )
            .with_unit("%")
            .with_value_to_string(Arc::new(|value| {
                format!("{:.0}", value * 100.0)
            }))
            .with_string_to_value(Arc::new(|string| {
                string.trim_end_matches("%")
                    .parse::<f32>()
                    .ok()
                    .map(|degrees| degrees / 100.0)
            })),
            brightness: FloatParam::new(
                "Brightness",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            )
            .with_unit("%")
            .with_value_to_string(Arc::new(|value| {
                format!("{:.0}", value * 100.0)
            }))
            .with_string_to_value(Arc::new(|string| {
                string.trim_end_matches("%")
                    .parse::<f32>()
                    .ok()
                    .map(|degrees| degrees / 100.0)
            })),
            immediate: BoolParam::new("Immediate", true),
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
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let hue = (self.params.hue.value() * 360.0) as u16;
        let saturation = (self.params.saturation.value() * 1000.0) as u16;
        let brightness = (self.params.brightness.value() * 1000.0) as u16;
        let immediate = self.params.immediate.value();

        if hue != self.last_hue || saturation != self.last_saturation || brightness != self.last_brightness {
            self.last_hue = hue;
            self.last_saturation = saturation;
            self.last_brightness = brightness;
            self.command_tx.send(BulbCommand::SetHSV(hue, saturation, brightness, immediate)).ok();
        }

        ProcessStatus::Normal
    }
}

fn bulb_controller_thread(command_rx: Receiver<BulbCommand>) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let mut controller = BulbController::new(BulbConfig::new(
            "eb052a1de220394996xwke",
            "!BY}~:dab1nuT;'n",
            "192.168.0.124",
            "3.3",
        )).unwrap();

        if controller.connect().await.is_ok() {
            nih_log!("Bulb connected successfully");
        } else {
            nih_error!("Failed to connect to bulb");
        }

        while let Ok(command) = command_rx.recv() {
            match command {
                BulbCommand::SetHSV(hue, saturation, brightness, immediate) => {
                    match controller.set_color(hue, saturation, brightness, immediate).await {
                        Ok(_) => {
                            nih_log!(
                                "Set bulb color to H:{} S:{} B:{}",
                                hue,
                                saturation,
                                brightness
                            );
                        }
                        Err(e) => {
                            nih_error!("Failed to set bulb color: {}", e);
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
