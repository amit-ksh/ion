//! Foreground display adapter. Native rasterization happens on a worker in waku-document.
use anyhow::Result;
pub const MAX_DOCUMENT_BYTES: usize = waku_document::MAX_DOCUMENT_BYTES;
use std::sync::Arc;
pub struct PdfRaster {
    pub page: u32,
    pub pages: u32,
    pub width: u32,
    pub height: u32,
    pub image: Arc<gpui::RenderImage>,
}
pub fn render_pdf(bytes: &[u8], page: u32, width: u32) -> Result<PdfRaster> {
    let raster = waku_document::render_pdf(bytes, page, width)?;
    let buffer = image::RgbaImage::from_raw(raster.width, raster.height, raster.bgra)
        .ok_or_else(|| anyhow::anyhow!("Invalid PDF pixel buffer"))?;
    Ok(PdfRaster {
        page: raster.page,
        pages: raster.pages,
        width: raster.width,
        height: raster.height,
        image: Arc::new(gpui::RenderImage::new(vec![image::Frame::new(buffer)])),
    })
}
