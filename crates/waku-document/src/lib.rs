//! Bounded native PDF rasterization shared by the owner and desktop.
//! Call on a worker: no filesystem, network, subprocess or authored document actions.
use anyhow::{Result, ensure};
pub const MAX_DOCUMENT_BYTES: usize = 64 * 1024;
pub const MAX_PAGES: u32 = 32;
pub const MAX_PIXELS: u64 = 3_200_000;
pub const MAX_PNG_BYTES: usize = 12 * 1024 * 1024;
pub struct PdfRaster {
    pub page: u32,
    pub pages: u32,
    pub width: u32,
    pub height: u32,
    /// Native display pixel order. Encoding is explicit and stays on the worker.
    pub bgra: Vec<u8>,
}
fn size(width: f64, height: f64, desired: u32) -> Result<(u32, u32)> {
    ensure!([800, 1200, 1600].contains(&desired), "Unsupported PDF zoom");
    ensure!(
        width.is_finite()
            && height.is_finite()
            && width > 0.0
            && height > 0.0
            && width <= 14400.0
            && height <= 14400.0,
        "Invalid PDF page dimensions"
    );
    let scale = (f64::from(desired) / width).min(2000.0 / height);
    let w = (width * scale).round().max(1.0) as u32;
    let h = (height * scale).round().max(1.0) as u32;
    ensure!(
        u64::from(w) * u64::from(h) <= MAX_PIXELS,
        "PDF page exceeds raster bound"
    );
    Ok((w, h))
}
// PdfPageRenderOptions dimensions are DIPs, while our decode/UI budget is pixels.
#[cfg(any(target_os = "windows", test))]
fn raster_dips(pixels: u32, dpi: u32) -> Result<u32> {
    ensure!((96..=960).contains(&dpi), "Unsupported PDF render DPI");
    let dips = ((u64::from(pixels) * 96) / u64::from(dpi)) as u32;
    ensure!(
        dips > 0,
        "PDF raster is smaller than one device-independent pixel"
    );
    Ok(dips)
}
fn image(page: u32, pages: u32, width: u32, height: u32, bgra: Vec<u8>) -> Result<PdfRaster> {
    ensure!(
        u64::from(width) * u64::from(height) <= MAX_PIXELS
            && bgra.len() == width as usize * height as usize * 4,
        "PDF raster dimensions changed"
    );
    Ok(PdfRaster {
        page,
        pages,
        width,
        height,
        bgra,
    })
}
impl PdfRaster {
    /// Full PNG derived from verified original bytes, with a separate transfer bound.
    pub fn png(&self) -> Result<Vec<u8>> {
        use image::ImageEncoder;
        ensure!(
            self.width > 0
                && self.width <= 1600
                && self.height > 0
                && self.height <= 2000
                && u64::from(self.width) * u64::from(self.height) <= MAX_PIXELS
                && self.bgra.len() == self.width as usize * self.height as usize * 4,
            "PDF raster dimensions changed before encoding"
        );
        let mut rgba = self.bgra.clone();
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png).write_image(
            &rgba,
            self.width,
            self.height,
            image::ExtendedColorType::Rgba8,
        )?;
        ensure!(
            png.len() <= MAX_PNG_BYTES,
            "PDF page transfer exceeds its bound"
        );
        Ok(png)
    }
}
pub struct PdfPng {
    pub page: u32,
    pub pages: u32,
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}
/// One process-wide nonblocking raster/encode admission for all phone paths.
/// Call only on an owner worker. Admission is released on error or unwind.
pub fn try_render_pdf_png(bytes: &[u8], page: u32, width: u32) -> Result<PdfPng> {
    static RENDER: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _permit = match RENDER.try_lock() {
        Ok(permit) => permit,
        Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => {
            anyhow::bail!("Another PDF page is rendering. Retry when it finishes.")
        }
    };
    let raster = render_pdf(bytes, page, width)?;
    Ok(PdfPng {
        page: raster.page,
        pages: raster.pages,
        width: raster.width,
        height: raster.height,
        png: raster.png()?,
    })
}
pub fn render_pdf(bytes: &[u8], page: u32, width: u32) -> Result<PdfRaster> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_DOCUMENT_BYTES && bytes.starts_with(b"%PDF-"),
        "PDF input exceeds the verified preview contract"
    );
    native(bytes, page, width)
}
#[cfg(target_os = "windows")]
fn native(bytes: &[u8], index: u32, desired: u32) -> Result<PdfRaster> {
    use windows::{
        Data::Pdf::{PdfDocument, PdfPageRenderOptions},
        Storage::Streams::{DataReader, DataWriter, InMemoryRandomAccessStream},
        Win32::{
            System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
            UI::HiDpi::{
                DPI_AWARENESS_CONTEXT_SYSTEM_AWARE, GetDpiForSystem, SetThreadDpiAwarenessContext,
            },
        },
    };
    use windows_future::AsyncStatus;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }?;
    let _apartment = Apartment;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    macro_rules! wait {
        ($value:expr) => {{
            let op = $value?;
            loop {
                if op.Status()? != AsyncStatus::Started {
                    break op.GetResults()?;
                }
                if std::time::Instant::now() >= deadline {
                    let _ = op.Cancel();
                    anyhow::bail!("PDF page rendering timed out");
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }};
    }
    let input = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&input)?;
    writer.WriteBytes(bytes)?;
    let written = wait!(writer.StoreAsync());
    ensure!(
        written as usize == bytes.len(),
        "PDF input stream is incomplete"
    );
    writer.DetachStream()?;
    input.Seek(0)?;
    let document = wait!(PdfDocument::LoadFromStreamAsync(&input));
    let pages = document.PageCount()?;
    ensure!(
        (1..=MAX_PAGES).contains(&pages) && index < pages,
        "PDF page selection is outside the supported 32-page document"
    );
    let page = document.GetPage(index)?;
    let dimensions = page.Size()?;
    let (width, height) = size(
        f64::from(dimensions.Width),
        f64::from(dimensions.Height),
        desired,
    )?;
    let output = InMemoryRandomAccessStream::new()?;
    let options = PdfPageRenderOptions::new()?;
    // Query real system DPI without changing the app or any window's awareness.
    // This synchronous worker scope restores the original thread context immediately.
    let previous = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE) };
    ensure!(!previous.0.is_null(), "PDF render DPI context unavailable");
    let dpi = unsafe { GetDpiForSystem() };
    let restored = unsafe { SetThreadDpiAwarenessContext(previous) };
    ensure!(
        !restored.0.is_null(),
        "PDF render DPI context could not be restored"
    );
    options.SetDestinationWidth(raster_dips(width, dpi)?)?;
    options.SetDestinationHeight(raster_dips(height, dpi)?)?;
    wait!(page.RenderWithOptionsToStreamAsync(&output, &options));
    let length = output.Size()?;
    ensure!(
        (1..=12 * 1024 * 1024).contains(&length),
        "PDF raster output exceeds its bound"
    );
    let reader = DataReader::CreateDataReader(&output.GetInputStreamAt(0)?)?;
    let loaded = wait!(reader.LoadAsync(length as u32));
    ensure!(
        u64::from(loaded) == length,
        "PDF raster output is incomplete"
    );
    let mut png = vec![0; length as usize];
    reader.ReadBytes(&mut png)?;
    ensure!(
        png.len() >= 24 && png[..8] == [137, 80, 78, 71, 13, 10, 26, 10] && &png[12..16] == b"IHDR",
        "PDF renderer did not return a PNG header"
    );
    let actual_width = u32::from_be_bytes(png[16..20].try_into()?);
    let actual_height = u32::from_be_bytes(png[20..24].try_into()?);
    ensure!(
        actual_width > 0 && actual_height > 0 && actual_width <= width && actual_height <= height,
        "PDF renderer exceeded pixel bounds: expected at most {width}x{height}, received {actual_width}x{actual_height}"
    );
    let mut decoder =
        image::ImageReader::with_format(std::io::Cursor::new(&png), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.max_alloc = Some(32 * 1024 * 1024);
    decoder.limits(limits);
    let mut rgba = decoder.decode()?.to_rgba8();
    ensure!(
        (rgba.width(), rgba.height()) == (actual_width, actual_height),
        "PDF decoder changed dimensions"
    );
    // Integer DIP rounding can leave at most a few pixels missing. Normalize on
    // the worker so UI zoom and memory bounds do not vary with display scaling.
    if (actual_width, actual_height) != (width, height) {
        rgba = image::imageops::resize(&rgba, width, height, image::imageops::FilterType::Triangle);
    }
    for pixel in rgba.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    image(index, pages, width, height, rgba.into_raw())
}
#[cfg(target_os = "macos")]
fn native(bytes: &[u8], index: u32, desired: u32) -> Result<PdfRaster> {
    use objc2_core_graphics::*;
    // The provider borrows bytes for this call; every CoreGraphics object is dropped before return.
    unsafe extern "C-unwind" fn release(
        _: *mut std::ffi::c_void,
        _: *const std::ffi::c_void,
        _: usize,
    ) {
    }
    let provider = unsafe {
        CGDataProvider::with_data(
            std::ptr::null_mut(),
            bytes.as_ptr().cast(),
            bytes.len(),
            Some(release),
        )
    }
    .ok_or_else(|| anyhow::anyhow!("PDF data provider unavailable"))?;
    let document = CGPDFDocument::with_provider(Some(&provider))
        .ok_or_else(|| anyhow::anyhow!("Invalid PDF document"))?;
    ensure!(
        !CGPDFDocument::is_encrypted(Some(&document)),
        "Encrypted PDF previews are unavailable"
    );
    let pages = u32::try_from(CGPDFDocument::number_of_pages(Some(&document)))?;
    ensure!(
        (1..=MAX_PAGES).contains(&pages) && index < pages,
        "PDF page selection is outside the supported 32-page document"
    );
    let page = CGPDFDocument::page(Some(&document), index as usize + 1)
        .ok_or_else(|| anyhow::anyhow!("PDF page unavailable"))?;
    let rect = CGPDFPage::box_rect(Some(&page), CGPDFBox::MediaBox);
    let rotation = CGPDFPage::rotation_angle(Some(&page));
    let (w, h) = if rotation.rem_euclid(180) == 90 {
        (rect.size.height, rect.size.width)
    } else {
        (rect.size.width, rect.size.height)
    };
    let (width, height) = size(w, h, desired)?;
    let mut data = vec![0u8; width as usize * height as usize * 4];
    let color = CGColorSpace::new_device_rgb()
        .ok_or_else(|| anyhow::anyhow!("PDF color space unavailable"))?;
    let context = unsafe {
        CGBitmapContextCreate(
            data.as_mut_ptr().cast(),
            width as usize,
            height as usize,
            8,
            width as usize * 4,
            Some(&color),
            CGImageAlphaInfo::PremultipliedFirst.0 | CGBitmapInfo::ByteOrder32Little.0,
        )
    }
    .ok_or_else(|| anyhow::anyhow!("PDF bitmap context unavailable"))?;
    let target = CGRect::new(
        CGPoint::new(0.0, 0.0),
        CGSize::new(f64::from(width), f64::from(height)),
    );
    CGContext::set_rgb_fill_color(Some(&context), 1.0, 1.0, 1.0, 1.0);
    CGContext::fill_rect(Some(&context), target);
    let transform = CGPDFPage::drawing_transform(Some(&page), CGPDFBox::MediaBox, target, 0, true);
    CGContext::concat_ctm(Some(&context), transform);
    CGContext::draw_pdf_page(Some(&context), Some(&page));
    drop(context);
    image(index, pages, width, height, data)
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn native(_: &[u8], _: u32, _: u32) -> Result<PdfRaster> {
    anyhow::bail!("Native PDF previews are unavailable on this platform")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pdf() -> Vec<u8> {
        let bodies = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << >> /Contents 4 0 R >>",
            "<< /Length 0 >>\nstream\n\nendstream",
        ];
        let mut bytes = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0];
        for (i, body) in bodies.iter().enumerate() {
            offsets.push(bytes.len());
            bytes.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
        }
        let start = bytes.len();
        bytes.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).bytes());
        for offset in offsets.iter().skip(1) {
            bytes.extend(format!("{offset:010} 00000 n \n").bytes());
        }
        bytes.extend(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n",
                offsets.len()
            )
            .bytes(),
        );
        bytes
    }
    #[test]
    fn png_preserves_color_channels_and_rejects_forged_raster_dimensions() {
        let mut raster = image(0, 1, 1, 1, vec![7, 23, 181, 255]).unwrap();
        let png = raster.png().unwrap();
        let decoded = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0, [181, 23, 7, 255]);
        raster.width = 1601;
        assert!(raster.png().is_err());
        raster.width = 1;
        raster.bgra.push(0);
        assert!(raster.png().is_err());
    }
    #[test]
    fn pdf_raster_dips_keep_scaled_output_within_pixel_budget() {
        for pixels in [800, 1200, 1600, 2000] {
            for dpi in [96, 120, 144, 168, 192, 240, 288, 384, 480, 960] {
                let dips = raster_dips(pixels, dpi).unwrap();
                assert!(u64::from(dips) * u64::from(dpi) <= u64::from(pixels) * 96);
            }
        }
        assert_eq!(raster_dips(800, 120).unwrap(), 640);
        assert!(raster_dips(0, 96).is_err());
        assert!(raster_dips(1, 120).is_err());
        assert!(raster_dips(800, 0).is_err());
        assert!(raster_dips(800, 961).is_err());
    }
    #[test]
    fn pdf_raster_bounds_reject_invalid_dimensions_and_zoom() {
        assert!(size(f64::NAN, 10.0, 1200).is_err());
        assert!(size(0.0, 10.0, 1200).is_err());
        assert!(size(10.0, 10.0, 10000).is_err());
        assert_eq!(size(200.0, 300.0, 1200).unwrap(), (1200, 1800));
    }
    #[test]
    fn pdf_raster_rejects_non_pdf_and_oversized_inputs() {
        assert!(render_pdf(b"not a PDF", 0, 1200).is_err());
        let mut bytes = vec![b' '; MAX_DOCUMENT_BYTES + 1];
        bytes[..5].copy_from_slice(b"%PDF-");
        assert!(render_pdf(&bytes, 0, 1200).is_err());
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[test]
    fn pdf_raster_native_page_metadata_and_selection() {
        let bytes = pdf();
        for zoom in [800, 1200, 1600] {
            let page = render_pdf(&bytes, 0, zoom).unwrap();
            let (width, height) = size(200.0, 300.0, zoom).unwrap();
            assert_eq!(
                (page.page, page.pages, page.width, page.height),
                (0, 1, width, height)
            );
            let png = page.png().unwrap();
            let decoded =
                image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (width, height));
            assert!(png.len() <= MAX_PNG_BYTES);
        }
        assert!(render_pdf(&bytes, 1, 800).is_err());
        assert!(render_pdf(b"%PDF-1.4\nmalformed\n%%EOF", 0, 800).is_err());
    }
}
