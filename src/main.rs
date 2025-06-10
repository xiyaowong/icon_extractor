#[cfg(not(windows))]
compile_error!("icon_extractor only supports Windows platform.");

use anyhow::Result;
use image::{ImageBuffer, Rgba};
use std::env;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::ptr::null_mut;
use tempfile::tempdir;
use winapi::shared::windef::HICON;
use winapi::um::shellapi::ExtractIconExW;
use winapi::um::wingdi::{BITMAP, BITMAPINFO, BITMAPINFOHEADER, GetObjectW};
use winapi::um::wingdi::{DIB_RGB_COLORS, GetDIBits};
use winapi::um::winuser::{DestroyIcon, GetDC, GetIconInfo, ReleaseDC};

pub fn extract_icon(file_path: &Path, output_dir: &Path) -> Result<PathBuf> {
    let satisfied = file_path.exists()
        && file_path
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("exe"));

    if !satisfied {
        anyhow::bail!(
            "The provided file is not a valid executable: {}",
            file_path.display()
        );
    }

    let target_path = file_path.to_path_buf();
    let file_str: Vec<u16> = target_path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();

    unsafe {
        let mut hicon_large: [HICON; 1] = [null_mut()];
        let extracted = ExtractIconExW(
            file_str.as_ptr(),
            0,
            hicon_large.as_mut_ptr(),
            null_mut(),
            1,
        );
        if extracted == 0 || hicon_large[0].is_null() {
            anyhow::bail!("ExtractIconExW failed for file: {}", target_path.display());
        }

        let hicon = hicon_large[0];

        let mut icon_info = std::mem::zeroed();
        if GetIconInfo(hicon, &mut icon_info) == 0 {
            DestroyIcon(hicon);
            anyhow::bail!("GetIconInfo failed.");
        }

        let mut bmp: BITMAP = std::mem::zeroed();
        if GetObjectW(
            icon_info.hbmColor as _,
            std::mem::size_of::<BITMAP>() as i32,
            &mut bmp as *mut _ as _,
        ) == 0
        {
            DestroyIcon(hicon);
            anyhow::bail!("GetObjectW failed.");
        }
        let width = bmp.bmWidth as usize;
        let height = bmp.bmHeight as usize;

        let mut bmp_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // 负表示自顶向下
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [std::mem::zeroed(); 1],
        };

        let mut pixels = vec![0u8; width * height * 4];

        let hdc = GetDC(null_mut());
        let ret = GetDIBits(
            hdc,
            icon_info.hbmColor,
            0,
            height as u32,
            pixels.as_mut_ptr() as _,
            &mut bmp_info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(null_mut(), hdc);

        if ret == 0 {
            DestroyIcon(hicon);
            anyhow::bail!("GetDIBits failed.");
        }

        let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(width as u32, height as u32, pixels)
                .ok_or_else(|| anyhow::anyhow!("Failed to create ImageBuffer"))?;

        let output_path = output_dir.join("icon.png");
        img.save(&output_path)?;

        DestroyIcon(hicon);

        winapi::um::wingdi::DeleteObject(icon_info.hbmColor as _);
        winapi::um::wingdi::DeleteObject(icon_info.hbmMask as _);

        Ok(output_path)
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        let exe = Path::new(&args[0])
            .file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_else(|| "icon_extractor".into());
        eprintln!(
            "Extract icons from executable files

Usage: {exe} <path-to-file>"
        );
        return Ok(());
    }

    let file_path = Path::new(&args[1]);
    let mut temp_dir = tempdir()?;
    temp_dir.disable_cleanup(true);

    let icon_path = extract_icon(file_path, temp_dir.path())?;
    _ = Command::new("explorer").arg(&icon_path).status();
    println!("Icon extracted to: {}", icon_path.display());

    Ok(())
}
