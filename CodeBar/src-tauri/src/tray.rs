//! 动态托盘图标:三段用量条(Rust 侧按用量实时渲染 32×32 RGBA);
//! 未接入任何 provider 时右上角红点角标。颜色取当前主题 accent。

use crate::config::Config;
use crate::models::{worst_window, ProviderSnapshot, ProviderStatus};
use tauri::image::Image;
use tauri::AppHandle;

const W: u32 = 32;
const H: u32 = 32;

pub fn accent_rgb(theme: &str) -> [u8; 3] {
    match theme {
        "mocha" => [0xcb, 0xa6, 0xf7],   // #cba6f7
        "latte" => [0x88, 0x39, 0xef],   // #8839ef
        _ => [0xe9, 0x65, 0xa5],         // hardhacker #e965a5
    }
}

const ERR_RGB: [u8; 3] = [0xe9, 0x30, 0x4e];

fn set_px(px: &mut [u8], x: u32, y: u32, rgb: [u8; 3]) {
    let i = ((y * W + x) * 4) as usize;
    px[i] = rgb[0];
    px[i + 1] = rgb[1];
    px[i + 2] = rgb[2];
    px[i + 3] = 255;
}

/// 三根竖条,高度按用量(取已接入 provider 中最紧张的三个窗口,降序)。
/// 无 provider → 三根最小高度条 + 红点。
pub fn render_icon(usages: &[f64], accent: [u8; 3], badge: bool) -> Image<'static> {
    let mut px = vec![0u8; (W * H * 4) as usize];
    let mut usages: Vec<f64> = usages.to_vec();
    usages.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    for (i, &u) in usages.iter().take(3).enumerate() {
        let x0 = 5 + i as u32 * 9; // 三根条:x 5-10 / 14-19 / 23-28
        let height = 4.0 + u.clamp(0.0, 100.0) / 100.0 * 20.0; // 4..24
        let height = height.round() as u32;
        for y in (27 - height)..27u32 {
            for x in x0..x0 + 5 {
                set_px(&mut px, x, y, accent);
            }
        }
    }
    if badge {
        // 右上角红点(r=4,圆心 26,6)
        for y in 2..11u32 {
            for x in 21..31u32 {
                let dx = x as i32 - 26;
                let dy = y as i32 - 6;
                if dx * dx + dy * dy <= 4 * 4 {
                    set_px(&mut px, x, y, ERR_RGB);
                }
            }
        }
    }
    Image::new_owned(px, W, H)
}

/// 刷新后更新托盘图标与 tooltip
pub fn update(app: &AppHandle, snapshots: &[ProviderSnapshot], cfg: &Config) {
    let usages: Vec<f64> = snapshots
        .iter()
        .filter(|s| matches!(s.status, ProviderStatus::Ok))
        .filter_map(|s| worst_window(&s.windows).and_then(|w| w.used_percent))
        .collect();
    let badge = snapshots.is_empty();
    let icon = render_icon(&usages, accent_rgb(&cfg.theme), badge);
    if let Some(tray) = app.tray_by_id("codebar-main") {
        let _ = tray.set_icon(Some(icon));
        let tip = if badge {
            "CodeBar — 未接入任何工具".to_string()
        } else {
            format!("CodeBar — {} 个工具已接入", snapshots.len())
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_renders_fully_opaque_bars() {
        let img = render_icon(&[80.0, 40.0, 10.0], [1, 2, 3], false);
        let rgba = img.rgba();
        assert_eq!(rgba.len(), (32 * 32 * 4) as usize);
        // 第一根条(最高 80%)底部像素非透明
        let i = ((26 * 32 + 7) * 4) as usize;
        assert_eq!(&rgba[i..i + 3], &[1, 2, 3]);
        assert_eq!(rgba[i + 3], 255);
        // 顶部空白区透明
        let i = ((1 * 32 + 7) * 4) as usize;
        assert_eq!(rgba[i + 3], 0);
    }

    #[test]
    fn badge_dot_present_when_empty() {
        let with_badge = render_icon(&[], [0, 0, 0], true);
        let i = ((6 * 32 + 26) * 4) as usize;
        assert_eq!(with_badge.rgba()[i + 3], 255);
        let no_badge = render_icon(&[50.0], [0, 0, 0], false);
        assert_eq!(no_badge.rgba()[i + 3], 0);
    }
}
