//! Unified Trivium JSON CLI & ultra-fast telemetry binary (`trivium-cli`).

mod devices;
mod eq_cmd;

use devices::{cmd_status, cmd_waybar_status, handle_fast_device_cmd};
use eq_cmd::{
    cmd_apply, cmd_autoeq_match, cmd_autoeq_target, cmd_bt_spp, cmd_export_apo, cmd_export_format,
    cmd_flat, cmd_import_apo, cmd_preset_delete, cmd_preset_save, cmd_search, cmd_source_fr,
    decode_b64, run_human_eq,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use trivium_core::config::DEFAULT_VIRTUAL_BANDS;

const BOOL_FLAGS: &[&str] = &[
    "--refresh-db",
    "--apply",
    "--commit",
    "--bypass",
    "--moondrop-only",
    "--in-ear-only",
    "--no-shelves",
    "--help",
    "-h",
    "--version",
    "-V",
];

pub(crate) struct CliArgs {
    pub(crate) cmd: String,
    pub(crate) pos: Vec<String>,
    pub(crate) kv: HashMap<String, String>,
    pub(crate) bools: HashSet<String>,
}

impl CliArgs {
    fn parse(raw_args: &[String]) -> Result<Self, String> {
        if raw_args.is_empty() {
            return Err("Subcomando requerido (eq | status | poll | waybar-status | apply | flat | search | source-fr | autoeq-match | autoeq-target | preset-save | preset-delete | import-apo | export-apo | bt-spp)".to_string());
        }
        let cmd = raw_args[0].clone();
        let mut pos = Vec::new();
        let mut kv = HashMap::new();
        let mut bools = HashSet::new();

        let mut i = 1;
        while i < raw_args.len() {
            let arg = &raw_args[i];
            if let Some((k, v)) = arg.split_once('=') {
                if BOOL_FLAGS.contains(&k) {
                    if v != "false" && v != "0" {
                        bools.insert(k.to_string());
                    }
                } else {
                    kv.insert(k.to_string(), v.to_string());
                }
                i += 1;
            } else if BOOL_FLAGS.contains(&arg.as_str()) {
                bools.insert(arg.clone());
                i += 1;
            } else if arg.starts_with("--") {
                if i + 1 < raw_args.len() {
                    kv.insert(arg.clone(), raw_args[i + 1].clone());
                    i += 2;
                } else {
                    kv.insert(arg.clone(), String::new());
                    i += 1;
                }
            } else {
                pos.push(arg.clone());
                i += 1;
            }
        }
        Ok(Self {
            cmd,
            pos,
            kv,
            bools,
        })
    }

    pub(crate) fn str_or(&self, key: &str, default: &str) -> String {
        self.kv
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    pub(crate) fn str_or_b64(&self, plain_key: &str, b64_key: &str) -> Result<String, String> {
        if let Some(b) = self.kv.get(b64_key) {
            if !b.is_empty() {
                return decode_b64(b);
            }
        }
        Ok(self.str_or(plain_key, ""))
    }

    pub(crate) fn has(&self, flag: &str) -> bool {
        self.bools.contains(flag)
    }

    pub(crate) fn f64_or(&self, key: &str, default: f64) -> f64 {
        self.kv
            .get(key)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(default)
    }

    pub(crate) fn opt_f64(&self, key: &str) -> Option<f64> {
        self.kv
            .get(key)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
    }

    pub(crate) fn usize_or(&self, key: &str, default: usize) -> usize {
        self.kv
            .get(key)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(default)
    }
}

fn dispatch(args: &CliArgs) -> Result<Value, String> {
    match args.cmd.as_str() {
        "status" => Ok(cmd_status(
            &args.str_or("--device-id", ""),
            args.has("--refresh-db"),
        )),
        "waybar-status" => Ok(cmd_waybar_status()),
        "source-fr" => {
            let src = args.str_or_b64("--src", "--src-b64")?;
            cmd_source_fr(&src)
        }
        "apply" => {
            let bj = args.str_or_b64("--bands-json", "--bands-b64")?;
            cmd_apply(
                &args.str_or("--device-id", ""),
                &bj,
                args.has("--commit"),
                !args.has("--bypass"),
                args.opt_f64("--preamp"),
                args.f64_or("--output-gain", 0.0),
            )
        }
        "flat" => cmd_flat(&args.str_or("--device-id", ""), args.has("--commit")),
        "search" => {
            let q = args.str_or_b64("--q", "--q-b64")?;
            Ok(cmd_search(
                &q,
                args.has("--moondrop-only"),
                args.has("--in-ear-only"),
                args.usize_or("--limit", 80),
            ))
        }
        "autoeq-match" => {
            let src = args.str_or_b64("--src", "--src-b64")?;
            let tgt = args.str_or_b64("--tgt", "--tgt-b64")?;
            cmd_autoeq_match(
                &src,
                &tgt,
                args.f64_or("--max-freq", 10_000.0),
                args.usize_or("--bands", DEFAULT_VIRTUAL_BANDS),
                args.f64_or("--max-gain", 12.0),
                !args.has("--no-shelves"),
            )
        }
        "autoeq-target" => {
            let src = args.str_or_b64("--src", "--src-b64")?;
            let tgt = args.str_or_b64("--target", "--target-b64")?;
            cmd_autoeq_target(
                &src,
                &tgt,
                args.f64_or("--max-freq", 10_000.0),
                args.usize_or("--bands", DEFAULT_VIRTUAL_BANDS),
                args.f64_or("--max-gain", 12.0),
                !args.has("--no-shelves"),
            )
        }
        "preset-save" => {
            let nm = args.str_or_b64("--name", "--name-b64")?;
            let bj = args.str_or_b64("--bands-json", "--bands-b64")?;
            cmd_preset_save(nm, &bj)
        }
        "preset-delete" => {
            let nm = args.str_or_b64("--name", "--name-b64")?;
            cmd_preset_delete(&nm)
        }
        "import-apo" => {
            let txt = decode_b64(&args.str_or("--text-b64", ""))?;
            Ok(cmd_import_apo(
                &txt,
                args.usize_or("--bands", DEFAULT_VIRTUAL_BANDS),
            ))
        }
        "export-apo" => {
            let bj = decode_b64(&args.str_or("--bands-b64", ""))?;
            cmd_export_apo(&bj, args.opt_f64("--preamp"))
        }
        "export-format" => {
            let fmt = args.str_or("--format", "apo").to_ascii_lowercase();
            let bj = args.str_or_b64("--bands-json", "--bands-b64")?;
            cmd_export_format(&fmt, &bj, args.opt_f64("--preamp"))
        }
        "bt-spp" => {
            let mac = args.str_or("--mac", "");
            let action = args.str_or("--action", "");
            let idx = args.usize_or("--preset-idx", 0) as u8;
            let bj = if args.kv.contains_key("--bands-b64") {
                args.str_or_b64("--bands-json", "--bands-b64")?
            } else {
                args.str_or("--bands-json", "[]")
            };
            cmd_bt_spp(&mac, &action, idx, &bj)
        }
        other => Err(format!("Comando no soportado: {other}")),
    }
}

fn print_help() {
    println!(
        "trivium-cli {} — Motor unificado en Rust para Trivium\n\
         Uso:\n\
           trivium-cli eq \"<audífono>\" [--target \"Harman In-Ear 2019\" | --match \"<otro>\"] [--apply] [--commit] [--out eq.txt] [--format apo|easyeffects|camilladsp|wavelet] [--output-gain <dB>]\n\
           trivium-cli <subcomando-json> [opciones]\n\n\
         Comandos interactivos de terminal:\n\
           eq               Descarga curva bajo demanda (con atribución), optimiza PEQ y exporta o aplica al hardware\n\
           hid-scan         Lista nodos HID/USB-CDC (KTMicro, MD8, Conexant, WalkPlay, FiiO, Qudelix, Topping, Fosi, JDS)\n\
                              Opciones: --all  --json  --dump-descriptors (reporte comunitario de hardware)\n\
           ble-protocols    Lista perfiles BLE GATT DSP soportados y sus UUIDs (FiiO EH11/EH13, Airoha/Maxwell)\n\n\
         Daemon residente opcional:\n\
           trivium-daemon   Escucha eventos udev/PipeWire y aplica automáticamente perfiles EQ por audífono\n\n\
         Subcomandos JSON / Telemetría (UI, Waybar, KDE Plasma):\n\
           poll             Huella instantánea (<1ms) de presencia USB HID / Bluetooth\n\
           waybar-status    Estado JSON compacto (<5ms) para módulo Waybar\n\
           status           Estado completo de dispositivos, bandas y catálogo AutoEQ\n\
           apply            Aplica bandas EQ (USB HID LIVE/COMMIT, PipeWire o Equalizer APO; --output-gain -30..12)\n\
           flat             Restablece todas las bandas a 0 dB\n\
           search           Busca entre +14,500 curvas AutoEq + Squiglink\n\
           source-fr        Reconstruye la respuesta en frecuencia stock de un modelo\n\
           autoeq-match     Calcula PEQ para imitar otro audífono del catálogo\n\
           autoeq-target    Calcula PEQ hacia una curva objetivo curada\n\
           preset-save      Guarda un preset de usuario\n\
           preset-delete    Elimina un preset de usuario\n\
           import-apo       Importa texto ParametricEQ.txt (Equalizer APO)\n\
           export-apo       Exporta bandas actuales a ParametricEQ.txt\n\
           export-format    Exporta bandas a apo, easyeffects, camilladsp o wavelet\n\
           bt-spp           Control opcional Bluetooth Classic SPP (preset/read/write)",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    let raw: Vec<String> = env::args().skip(1).collect();
    if raw.is_empty() || matches!(raw[0].as_str(), "-h" | "--help" | "help") {
        print_help();
        return;
    }
    if matches!(raw[0].as_str(), "-V" | "--version" | "version") {
        println!("trivium-cli {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if handle_fast_device_cmd(&raw) {
        return;
    }
    if raw[0] == "eq" {
        if raw.iter().skip(1).any(|a| a == "-h" || a == "--help") {
            print_help();
            return;
        }
        if let Err(e) = CliArgs::parse(&raw).and_then(|a| run_human_eq(&a)) {
            eprintln!("[ERROR] {e}");
            std::process::exit(1);
        }
        return;
    }

    let out = match CliArgs::parse(&raw).and_then(|a| dispatch(&a)) {
        Ok(v) => v,
        Err(e) => json!({ "ok": false, "error": e }),
    };

    if let Ok(line) = serde_json::to_string(&out) {
        println!("{line}");
    } else {
        println!(r#"{{"ok":false,"error":"Error serializando salida JSON"}}"#);
    }
}
