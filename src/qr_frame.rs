/// QR Frame — QR code encoding and Frame Snap PNG generation
///
/// Encodes CryoFrame into one or more QR payloads with sequence headers.
/// Generates a 512x512 PNG "Frame Snap" with:
///   - Synced background (system-state-dependent color gradient)
///   - Wave memory overlay (sinusoidal patterns from spectral data)
///   - Flag badges (visual status indicators)
///   - Spectral heatmap + core usage bars
///   - QR codes + metadata + BLAKE3 footer

use crate::cryo::{cryo_flags, CryoFrame, SpectralSlice};
use image::{Rgba, RgbaImage};
use qrcode::QrCode;

// ── QR Encoding ──

const QR_MAGIC: u8 = 0xCF; // CryoFrame marker
const MAX_QR_PAYLOAD: usize = 2000;

/// Encode a CryoFrame into QR payloads with sequence headers
/// Header: [0xCF, flags, index, total, hash_byte0..3]  (8 bytes)
pub fn encode_qr_payloads(frame: &CryoFrame) -> Vec<Vec<u8>> {
    let raw = crate::cryo::encode_binary(frame);
    let compressed = crate::cryo::compress(&raw);

    let hash_bytes: Vec<u8> = frame
        .frame_hash
        .as_bytes()
        .chunks(2)
        .take(4)
        .filter_map(|c| {
            std::str::from_utf8(c)
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
        })
        .collect();

    let hash4: [u8; 4] = [
        hash_bytes.first().copied().unwrap_or(0),
        hash_bytes.get(1).copied().unwrap_or(0),
        hash_bytes.get(2).copied().unwrap_or(0),
        hash_bytes.get(3).copied().unwrap_or(0),
    ];

    let data_per_qr = MAX_QR_PAYLOAD - 8; // 8 bytes header
    let chunks: Vec<&[u8]> = compressed.chunks(data_per_qr).collect();
    let total = chunks.len() as u8;

    // Set MULTI_QR flag if needed
    let qr_flags = if total > 1 {
        frame.flags | cryo_flags::MULTI_QR
    } else {
        frame.flags & !cryo_flags::MULTI_QR
    };

    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut payload = Vec::with_capacity(8 + chunk.len());
            payload.push(QR_MAGIC);
            payload.push(qr_flags);
            payload.push(i as u8);
            payload.push(total);
            payload.extend_from_slice(&hash4);
            payload.extend_from_slice(chunk);
            payload
        })
        .collect()
}

/// Decode QR payloads back into a CryoFrame
pub fn decode_qr_payloads(payloads: &[Vec<u8>]) -> Result<CryoFrame, String> {
    if payloads.is_empty() {
        return Err("no payloads".into());
    }

    let mut sorted: Vec<(u8, &[u8])> = Vec::new();
    for p in payloads {
        if p.len() < 8 || p[0] != QR_MAGIC {
            return Err("invalid QR payload header".into());
        }
        let idx = p[2];
        let data = &p[8..];
        sorted.push((idx, data));
    }
    sorted.sort_by_key(|(idx, _)| *idx);

    let mut compressed = Vec::new();
    for (_, data) in sorted {
        compressed.extend_from_slice(data);
    }

    let raw = crate::cryo::decompress(&compressed)
        .map_err(|e| format!("decompress: {}", e))?;
    crate::cryo::decode_binary(&raw).map_err(|e| format!("decode: {}", e))
}

// ── Frame Snap PNG ──

const SNAP_W: u32 = 512;
const SNAP_H: u32 = 512;

/// Generate a Frame Snap PNG (512x512 RGBA) with synced background,
/// wave memory overlay, flag badges, heatmap, QR codes.
pub fn generate_frame_snap(frame: &CryoFrame) -> Vec<u8> {
    let mut img = RgbaImage::new(SNAP_W, SNAP_H);

    // ── 1. Synced background ──
    render_synced_background(&mut img, frame);

    // ── 2. Wave memory overlay ──
    render_wave_memory(&mut img, frame);

    // ── 3. Title bar (0..32) ──
    let title = format!("CRYOSTASIS v{} -- {}", frame.version, frame.hostname);
    render_text(&mut img, &title, 8, 4, Rgba([0, 220, 255, 255]));
    render_text(
        &mut img,
        &format!("origin: {} gen={}", frame.origin_binary, frame.generation),
        8,
        16,
        Rgba([120, 180, 200, 255]),
    );

    // ── 4. Flag badges (top-right) ──
    render_flag_badges(&mut img, frame.flags);

    // Separator line
    for x in 0..SNAP_W {
        img.put_pixel(x, 31, Rgba([0, 100, 140, 200]));
    }

    // ── 5. Spectral heatmap (32..288 = 256px) ──
    render_heatmap(&mut img, &frame.spectral_slices, &frame.cores, 32, 288);

    // Separator
    for x in 0..SNAP_W {
        img.put_pixel(x, 288, Rgba([0, 100, 140, 200]));
    }

    // ── 6. QR code + metadata region (289..496) ──
    let qr_payloads = encode_qr_payloads(frame);
    if let Some(first_payload) = qr_payloads.first() {
        if let Ok(qr) = QrCode::new(first_payload) {
            render_qr(&mut img, &qr, 16, 300, 2);
            render_qr(&mut img, &qr, 340, 300, 2);
        }
    }

    // Metadata text in center
    let meta_x = 180;
    render_text(
        &mut img,
        &frame.frozen_at[..19.min(frame.frozen_at.len())],
        meta_x,
        310,
        Rgba([200, 200, 200, 255]),
    );
    render_text(
        &mut img,
        &format!("res: {:.1}", frame.resonance_score_x10 as f64 / 10.0),
        meta_x,
        325,
        Rgba([0, 255, 200, 255]),
    );
    render_text(
        &mut img,
        &format!("stb: {:.3}", frame.stability_index_x1000 as f64 / 1000.0),
        meta_x,
        340,
        Rgba([0, 200, 255, 255]),
    );
    render_text(
        &mut img,
        &format!(
            "cpu: {:.1}%  mem: {}/{}MB",
            frame.cpu_global_x10 as f64 / 10.0,
            frame.memory_used_mb,
            frame.memory_total_mb
        ),
        meta_x,
        355,
        Rgba([180, 180, 180, 255]),
    );
    render_text(
        &mut img,
        &format!("cores: {}  procs: {}", frame.cores.len(), frame.process_count),
        meta_x,
        370,
        Rgba([180, 180, 180, 255]),
    );
    // Wave memory count
    if !frame.wave_memory.is_empty() {
        render_text(
            &mut img,
            &format!("waves: {}", frame.wave_memory.len()),
            meta_x,
            385,
            Rgba([140, 100, 255, 255]),
        );
    }
    if !frame.drone_names.is_empty() {
        let y = if frame.wave_memory.is_empty() { 385 } else { 400 };
        render_text(
            &mut img,
            &format!("drones: {}", frame.drone_names.len()),
            meta_x,
            y,
            Rgba([180, 220, 100, 255]),
        );
    }

    // Flags text
    let flag_names = cryo_flags::names(frame.flags);
    if !flag_names.is_empty() {
        let flags_str = format!("flags: {}", flag_names.join(" "));
        render_text(&mut img, &flags_str, meta_x, 420, Rgba([100, 140, 180, 255]));
    }

    // Footer separator
    for x in 0..SNAP_W {
        img.put_pixel(x, 496, Rgba([0, 100, 140, 200]));
    }

    // Footer: BLAKE3 hash
    let hash_short = if frame.frame_hash.len() > 48 {
        &frame.frame_hash[..48]
    } else {
        &frame.frame_hash
    };
    render_text(
        &mut img,
        &format!("BLAKE3: {}", hash_short),
        8,
        500,
        Rgba([100, 100, 120, 255]),
    );

    // Encode to PNG
    let mut png_buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        SNAP_W,
        SNAP_H,
        image::ExtendedColorType::Rgba8,
    )
    .unwrap_or(());
    png_buf
}

// ── Synced background ──
// Background color reflects system state:
//   Low CPU + high stability → deep blue/purple (calm)
//   High CPU → amber/red tint (stressed)
//   Anomaly flag → red pulse
//   System freeze → purple glow

fn render_synced_background(img: &mut RgbaImage, frame: &CryoFrame) {
    let cpu = frame.cpu_global_x10 as f64 / 10.0;     // 0..100
    let stability = frame.stability_index_x1000 as f64 / 1000.0; // 0..1
    let resonance = frame.resonance_score_x10 as f64 / 1000.0;   // 0..~1
    let has_anomaly = frame.flags & cryo_flags::SPECTRAL_ANOMALY != 0;
    let is_system = frame.flags & cryo_flags::SYSTEM_FREEZE != 0;

    // Base color selection
    let (base_r, base_g, base_b) = if has_anomaly {
        // Red-tinted — spectral anomaly
        (35, 8, 12)
    } else if cpu > 80.0 {
        // Amber — high CPU stress
        (30, 18, 5)
    } else if is_system {
        // Purple — system-wide freeze
        (18, 8, 30)
    } else {
        // Deep blue — normal/calm
        (6, 8, 22)
    };

    for y in 0..SNAP_H {
        for x in 0..SNAP_W {
            // Vertical gradient: darker at top, slightly lighter at bottom
            let vert = y as f64 / SNAP_H as f64;
            let grad = 0.7 + vert * 0.3;

            // Radial vignette from center
            let cx = (x as f64 - SNAP_W as f64 / 2.0) / (SNAP_W as f64 / 2.0);
            let cy = (y as f64 - SNAP_H as f64 / 2.0) / (SNAP_H as f64 / 2.0);
            let dist = (cx * cx + cy * cy).sqrt().min(1.4);
            let vignette = 1.0 - dist * 0.3;

            // Resonance ripple — concentric rings from center
            let ripple = ((dist * 8.0 * std::f64::consts::PI + resonance * 20.0).sin() * 0.5 + 0.5)
                * stability * 0.15;

            let factor = grad * vignette + ripple;
            let r = (base_r as f64 * factor).clamp(0.0, 255.0) as u8;
            let g = (base_g as f64 * factor).clamp(0.0, 255.0) as u8;
            let b = (base_b as f64 * factor).clamp(0.0, 255.0) as u8;

            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
}

// ── Wave memory overlay ──
// Renders sinusoidal waves from wave_memory data as semi-transparent overlays.
// Each wave is drawn with its frequency (controls period), amplitude (controls height),
// and phase (controls offset). Band determines color.

fn render_wave_memory(img: &mut RgbaImage, frame: &CryoFrame) {
    if frame.wave_memory.is_empty() {
        return;
    }

    let wave_count = frame.wave_memory.len();

    for (wi, wave) in frame.wave_memory.iter().enumerate() {
        // Distribute waves vertically across the image
        let center_y = 60.0 + (wi as f64 / wave_count as f64) * 400.0;

        // Wave color based on band
        let (wr, wg, wb) = match wave.band.as_str() {
            "hw"   => (0, 80, 160),    // Deep blue
            "net"  => (0, 140, 120),    // Teal
            "task" => (120, 60, 180),   // Purple
            "core" => (80, 140, 60),    // Green
            b if b.starts_with("emoti:") => (220, 180, 40), // Gold
            _      => (60, 60, 100),    // Gray-blue
        };

        // Normalize frequency to visual period (pixels per cycle)
        // Map frequency logarithmically to 20..200 pixel period
        let freq_norm = if wave.frequency_hz > 0.0 {
            (wave.frequency_hz.ln() + 1.0).clamp(1.0, 10.0) / 10.0
        } else {
            0.5
        };
        let period = 20.0 + (1.0 - freq_norm) * 180.0;

        // Amplitude mapped to pixel height (5..40 px)
        let amp_px = (wave.amplitude * 40.0).clamp(5.0, 40.0);

        // Alpha: semi-transparent (emoti bands are stronger)
        let alpha = if wave.band.starts_with("emoti:") { 55_u8 } else { 35_u8 };

        for x in 0..SNAP_W {
            let xf = x as f64;
            let theta = (xf / period) * 2.0 * std::f64::consts::PI + wave.phase_rad;
            let wave_y = center_y + theta.sin() * amp_px;

            // Draw 2px thick wave line
            for dy in 0..2 {
                let py = (wave_y as i32 + dy) as u32;
                if py < SNAP_H {
                    let existing = img.get_pixel(x, py);
                    let blended = alpha_blend(existing, Rgba([wr, wg, wb, alpha]));
                    img.put_pixel(x, py, blended);
                }
            }
        }

        // Draw a small frequency marker at the left edge
        let marker_y = center_y as u32;
        if marker_y + 3 < SNAP_H {
            for dy in 0..3 {
                for dx in 0..3 {
                    if dx < SNAP_W && marker_y + dy < SNAP_H {
                        img.put_pixel(dx, marker_y + dy, Rgba([wr, wg, wb, 80]));
                    }
                }
            }
        }
    }
}

/// Alpha-blend a foreground color over an existing pixel
pub fn alpha_blend(bg: &Rgba<u8>, fg: Rgba<u8>) -> Rgba<u8> {
    let a = fg[3] as f32 / 255.0;
    let inv_a = 1.0 - a;
    Rgba([
        (fg[0] as f32 * a + bg[0] as f32 * inv_a) as u8,
        (fg[1] as f32 * a + bg[1] as f32 * inv_a) as u8,
        (fg[2] as f32 * a + bg[2] as f32 * inv_a) as u8,
        255,
    ])
}

// ── Flag badges ──
// Renders colored rectangles with labels in the top-right corner

fn render_flag_badges(img: &mut RgbaImage, flags: u8) {
    let badge_defs: Vec<(u8, &str, Rgba<u8>)> = vec![
        (cryo_flags::INTEGRITY_OK,     "OK",      Rgba([0, 180, 80, 255])),
        (cryo_flags::SPECTRAL_ANOMALY, "ANOMALY", Rgba([220, 60, 40, 255])),
        (cryo_flags::DRIFT_WARNING,    "DRIFT",   Rgba([220, 160, 0, 255])),
        (cryo_flags::SYSTEM_FREEZE,    "SYS",     Rgba([140, 60, 220, 255])),
        (cryo_flags::MANUAL_FREEZE,    "MAN",     Rgba([60, 140, 200, 255])),
        (cryo_flags::WAVE_MEMORY,      "WAV",     Rgba([100, 80, 200, 255])),
        (cryo_flags::COMPRESSED,       "ZIP",     Rgba([80, 120, 120, 255])),
        (cryo_flags::MULTI_QR,         "MQR",     Rgba([160, 120, 60, 255])),
    ];

    let mut badge_x: u32 = SNAP_W - 8;

    for (flag_bit, label, color) in &badge_defs {
        if flags & flag_bit == 0 {
            continue;
        }

        let label_w = (label.len() as u32) * 6 + 4; // 6px per char + 4px padding
        if badge_x < label_w + 4 {
            break;
        }
        badge_x -= label_w + 4;

        // Draw badge background (rounded-ish rectangle)
        let badge_y = 3_u32;
        let badge_h = 10_u32;
        for by in 0..badge_h {
            for bx in 0..label_w {
                let px = badge_x + bx;
                let py = badge_y + by;
                if px < SNAP_W && py < SNAP_H {
                    // Dimmed background
                    let bg = Rgba([color[0] / 4, color[1] / 4, color[2] / 4, 200]);
                    img.put_pixel(px, py, bg);
                }
            }
        }
        // Top and bottom border lines
        for bx in 0..label_w {
            let px = badge_x + bx;
            if px < SNAP_W {
                img.put_pixel(px, badge_y, *color);
                img.put_pixel(px, badge_y + badge_h - 1, *color);
            }
        }

        // Render label text
        render_text(img, label, badge_x + 2, badge_y + 2, *color);
    }
}

// ── Heatmap rendering ──

fn render_heatmap(
    img: &mut RgbaImage,
    slices: &[SpectralSlice],
    cores: &[crate::cryo::CoreFreeze],
    y_start: u32,
    y_end: u32,
) {
    let height = y_end - y_start;

    // Upper half: spectral slices heatmap
    let slice_h = (height / 2).min(128);
    let rows_per_band = slice_h / 3.max(slices.len() as u32);

    for (band_idx, slice) in slices.iter().enumerate() {
        let row_y = y_start + (band_idx as u32) * rows_per_band;
        let amps: Vec<f32> = slice.data.iter().step_by(2).copied().collect();

        if amps.is_empty() {
            continue;
        }

        let max_amp = amps.iter().cloned().fold(0.0_f32, f32::max).max(0.001);

        // Band label
        render_text(img, &slice.band_id, 4, row_y + 2, Rgba([80, 80, 100, 255]));

        let bar_start_x = 40_u32;
        let bar_w = (SNAP_W - bar_start_x - 8) as usize;

        for row in 0..rows_per_band.min(20) {
            for col in 0..bar_w {
                let bin_idx = (col * amps.len()) / bar_w;
                if bin_idx < amps.len() {
                    let intensity = (amps[bin_idx] / max_amp).clamp(0.0, 1.0);
                    let color = heatmap_color(intensity);
                    let px = bar_start_x + col as u32;
                    let py = row_y + row;
                    if px < SNAP_W && py < y_end {
                        let existing = img.get_pixel(px, py);
                        let blended = alpha_blend(existing, Rgba([color[0], color[1], color[2], 200]));
                        img.put_pixel(px, py, blended);
                    }
                }
            }
        }
    }

    // Lower half: core usage bars
    let core_y_start = y_start + slice_h + 8;
    let available_h = y_end.saturating_sub(core_y_start).saturating_sub(8);
    let core_count = cores.len().max(1);
    let bar_h = (available_h / core_count as u32).min(8).max(2);

    render_text(
        img,
        "CORE USAGE",
        4,
        core_y_start,
        Rgba([80, 80, 100, 255]),
    );

    for (i, core) in cores.iter().enumerate() {
        let cy = core_y_start + 12 + (i as u32) * (bar_h + 1);
        if cy + bar_h >= y_end {
            break;
        }

        let usage = core.usage as f64 / 10.0 / 100.0; // 0..1
        let bar_w = ((SNAP_W - 50) as f64 * usage) as u32;

        for row in 0..bar_h {
            for col in 0..bar_w {
                let px = 40 + col;
                let py = cy + row;
                if px < SNAP_W && py < y_end {
                    let color = heatmap_color(usage as f32);
                    let existing = img.get_pixel(px, py);
                    let blended = alpha_blend(existing, Rgba([color[0], color[1], color[2], 220]));
                    img.put_pixel(px, py, blended);
                }
            }
        }
    }
}

/// Blue -> Cyan -> White gradient
pub fn heatmap_color(intensity: f32) -> Rgba<u8> {
    let t = intensity.clamp(0.0, 1.0);
    if t < 0.5 {
        let f = t * 2.0;
        Rgba([0, (f * 200.0) as u8, (100.0 + f * 155.0) as u8, 255])
    } else {
        let f = (t - 0.5) * 2.0;
        Rgba([
            (f * 255.0) as u8,
            (200.0 + f * 55.0) as u8,
            255,
            255,
        ])
    }
}

// ── QR rendering ──

fn render_qr(img: &mut RgbaImage, qr: &QrCode, x_off: u32, y_off: u32, scale: u32) {
    let modules = qr.to_colors();
    let width = qr.width() as u32;

    for (idx, &color) in modules.iter().enumerate() {
        let qx = (idx as u32) % width;
        let qy = (idx as u32) / width;

        let pixel_color = match color {
            qrcode::Color::Dark => Rgba([220, 220, 240, 255]),
            qrcode::Color::Light => Rgba([10, 10, 20, 255]),
        };

        for dy in 0..scale {
            for dx in 0..scale {
                let px = x_off + qx * scale + dx;
                let py = y_off + qy * scale + dy;
                if px < SNAP_W && py < SNAP_H {
                    img.put_pixel(px, py, pixel_color);
                }
            }
        }
    }
}

// ── Bitmap font (5x7) ──

pub fn render_text(img: &mut RgbaImage, text: &str, x: u32, y: u32, color: Rgba<u8>) {
    let mut cx = x;
    for ch in text.chars() {
        if let Some(glyph) = get_glyph(ch) {
            for (row, &bits) in glyph.iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        let px = cx + col;
                        let py = y + row as u32;
                        if px < SNAP_W && py < SNAP_H {
                            img.put_pixel(px, py, color);
                        }
                    }
                }
            }
        }
        cx += 6;
    }
}

fn get_glyph(ch: char) -> Option<&'static [u8; 7]> {
    let ch = ch.to_ascii_uppercase();
    match ch {
        'A' => Some(&[0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'B' => Some(&[0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110]),
        'C' => Some(&[0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
        'D' => Some(&[0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100]),
        'E' => Some(&[0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
        'F' => Some(&[0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
        'G' => Some(&[0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110]),
        'H' => Some(&[0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'I' => Some(&[0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'J' => Some(&[0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
        'K' => Some(&[0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
        'L' => Some(&[0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
        'M' => Some(&[0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
        'N' => Some(&[0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
        'O' => Some(&[0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'P' => Some(&[0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
        'Q' => Some(&[0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b01110, 0b00001]),
        'R' => Some(&[0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
        'S' => Some(&[0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110]),
        'T' => Some(&[0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        'U' => Some(&[0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'V' => Some(&[0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
        'W' => Some(&[0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]),
        'X' => Some(&[0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
        'Y' => Some(&[0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
        'Z' => Some(&[0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
        '0' => Some(&[0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
        '1' => Some(&[0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        '2' => Some(&[0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111]),
        '3' => Some(&[0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110]),
        '4' => Some(&[0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
        '5' => Some(&[0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
        '6' => Some(&[0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
        '7' => Some(&[0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
        '8' => Some(&[0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
        '9' => Some(&[0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110]),
        ' ' => Some(&[0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
        ':' => Some(&[0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000]),
        '.' => Some(&[0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100]),
        '-' => Some(&[0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
        '_' => Some(&[0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111]),
        '/' => Some(&[0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000]),
        '=' => Some(&[0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000]),
        '%' => Some(&[0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011]),
        '(' => Some(&[0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010]),
        ')' => Some(&[0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000]),
        '+' => Some(&[0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]),
        '*' => Some(&[0b00000, 0b10101, 0b01110, 0b00100, 0b01110, 0b10101, 0b00000]),
        '#' => Some(&[0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0b00000]),
        '>' => Some(&[0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000]),
        '<' => Some(&[0b00001, 0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0b00001]),
        '|' => Some(&[0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        _ => None,
    }
}
