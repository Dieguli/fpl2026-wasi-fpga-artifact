//! Host Rust que decodifica .avi y ejecuta inferencia BCPNN via WASM
//!
//! Transparente para el usuario: acepta .avi directamente
//! 
//! Compilar:
//!   cargo build --release --bin avi_processor
//!
//! Usar:
//!   ./target/release/avi_processor video.avi xclbin_path weights_path
//!
//! El programa:
//!   1. Detecta si el input es .avi
//!   2. Decodifica frames con ffmpeg (via proceso externo)
//!   3. Genera archivo .bin temporal (28x28 escala de grises)
//!   4. Llama a wasmedge con ese .bin
//!   5. Limpia archivos temporales

use std::process::{Command, exit};
use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 4 {
        eprintln!("Uso: {} <video.avi> <xclbin_path> <weights_path>", args[0]);
        eprintln!("  o: {} --synthetic <xclbin_path> <weights_path>", args[0]);
        exit(1);
    }

    let video_input = &args[1];
    let xclbin_path = &args[2];
    let weights_path = &args[3];

    println!("=== BCPNN Video Processor (Rust Host + WASM) ===\n");

    // Verificar que xclbin y weights existen
    if !Path::new(xclbin_path).exists() {
        eprintln!(" xclbin no encontrado: {}", xclbin_path);
        exit(1);
    }
    if !Path::new(weights_path).exists() {
        eprintln!(" weights no encontrado: {}", weights_path);
        exit(1);
    }

    let bin_file = if video_input == "--synthetic" {
        println!("[1] Modo sintético: generando 10 frames (dígitos 0-9)");
        // Pasar None al WASM (modo sintético)
        None
    } else if video_input.ends_with(".avi") {
        println!("[1] Detectado archivo .avi: {}", video_input);
        
        if !Path::new(video_input).exists() {
            eprintln!(" Video no encontrado: {}", video_input);
            exit(1);
        }

        let bin_path = "/tmp/bcpnn_frames.bin";
        println!("[2] Decodificando frames con ffmpeg...");
        
        // Comando ffmpeg: extrae frames 28x28 en escala de grises
        let status = Command::new("ffmpeg")
            .arg("-i").arg(video_input)
            .arg("-vf").arg("scale=28:28,format=gray")
            .arg("-f").arg("rawvideo")
            .arg("-pix_fmt").arg("gray")
            .arg("-")
            .output();

        match status {
            Ok(output) => {
                if !output.status.success() {
                    eprintln!(" ffmpeg falló");
                    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                    exit(1);
                }

                // Convertir frames raw a floats [0,1] y guardar en .bin
                let raw_frames = output.stdout;
                let num_frames = raw_frames.len() / (28 * 28);
                
                println!("  Frames extraídos: {}", num_frames);
                println!("  Convirtiendo a formato float (28x28→1568 floats)...");

                let mut bin_data = Vec::new();

                for frame_idx in 0..num_frames {
                    let offset = frame_idx * 28 * 28;
                    let frame_raw = &raw_frames[offset..offset + 28*28];

                    // Convertir 28x28 píxeles a 1568 floats (complementario)
                    let mut inputdata = vec![0.0f32; 1568];

                    for i in 0..784 {
                        let pixel = frame_raw[i] as f32 / 255.0;
                        inputdata[2 * i]     = 1.0 - pixel;  // complementario
                        inputdata[2 * i + 1] = pixel;
                    }

                    // Guardar en .bin (cada float como 4 bytes LE)
                    for &val in &inputdata {
                        bin_data.extend_from_slice(&val.to_le_bytes());
                    }
                }

                // Guardar archivo .bin
                match fs::write(bin_path, &bin_data) {
                    Ok(_) => {
                        println!("   {} frames guardados en {}", num_frames, bin_path);
                        Some(bin_path.to_string())
                    }
                    Err(e) => {
                        eprintln!(" Error escribiendo {}: {}", bin_path, e);
                        exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!(" ffmpeg no disponible: {}", e);
                eprintln!("   Instala con: apt-get install ffmpeg");
                exit(1);
            }
        }
    } else {
        eprintln!(" Archivo debe ser .avi o usar --synthetic");
        exit(1);
    };

    // ===========================================================================
    // Ejecutar módulo WASM
    // ===========================================================================
    println!("[3] Ejecutando inferencia BCPNN via WASM...\n");

    let mut wasmedge_args = vec![
        "--dir".to_string(),
        ".:".to_string(),
        "wasm/test_bcpnn_infer_video.wasm".to_string(),
        xclbin_path.to_string(),
        weights_path.to_string(),
    ];

    // Si tenemos .bin, pasarlo; si no (modo sintético), no lo pasamos
    if let Some(bin_file) = &bin_file {
        wasmedge_args.push(bin_file.clone());
    }

    let status = Command::new("wasmedge")
        .args(&wasmedge_args)
        .status();

    match status {
        Ok(exit_status) => {
            if !exit_status.success() {
                eprintln!(" WASM execution failed");
                exit(1);
            }
        }
        Err(e) => {
            eprintln!(" wasmedge no disponible: {}", e);
            eprintln!("   Instala WasmEdge: https://wasmedge.org/");
            exit(1);
        }
    }

    // ===========================================================================
    // Cleanup
    // ===========================================================================
    if let Some(bin_file) = bin_file {
        println!("\n[4] Limpiando archivos temporales...");
        if let Err(e) = fs::remove_file(&bin_file) {
            eprintln!("  Advertencia: no se pudo borrar {}: {}", bin_file, e);
        } else {
            println!("   {} eliminado", bin_file);
        }
    }

    println!("\n=== Procesamiento Completo ===");
}
