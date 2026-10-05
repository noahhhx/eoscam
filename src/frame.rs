use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

const BLACK: [u8; 4] = [16, 128, 16, 128];

fn yuyv_aligned(x: usize) -> usize {
    x & !1
}

pub struct Converter {
    width: usize,
    height: usize,
    out: Vec<u8>,
    ycbcr: Vec<u8>,
    last_src_size: Option<(usize, usize)>,
    /// JPEG is full-range YCbCr; webcam consumers expect limited (BT.601) range.
    luma: [u8; 256],
    chroma: [u8; 256],
}

impl Converter {
    pub fn new(width: u32, height: u32) -> Self {
        let (width, height) = (width as usize, height as usize);
        Self {
            width,
            height,
            out: BLACK.repeat(width * height / 2),
            ycbcr: Vec::new(),
            last_src_size: None,
            luma: std::array::from_fn(|v| (16 + v * 219 / 255) as u8),
            chroma: std::array::from_fn(|v| (128 + (v as i32 - 128) * 224 / 255) as u8),
        }
    }

    pub fn black(&mut self) -> &[u8] {
        self.fill_black();
        &self.out
    }

    pub fn convert(&mut self, jpeg: &[u8]) -> Result<&[u8], zune_jpeg::errors::DecodeErrors> {
        let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::YCbCr);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(jpeg), options);
        decoder.decode_headers()?;
        let info = decoder.info().expect("headers decoded");
        let (src_w, src_h) = (usize::from(info.width), usize::from(info.height));
        self.ycbcr
            .resize(decoder.output_buffer_size().expect("headers decoded"), 0);
        decoder.decode_into(&mut self.ycbcr)?;

        if self.last_src_size != Some((src_w, src_h)) {
            if (src_w, src_h) != (self.width, self.height) {
                eprintln!(
                    "camera frames are {src_w}x{src_h} but output is {}x{}; cropping/padding \
                     (pass --size {src_w}x{src_h} to match)",
                    self.width, self.height
                );
            }
            self.fill_black();
            self.last_src_size = Some((src_w, src_h));
        }

        let copy_w = yuyv_aligned(src_w.min(self.width));
        let copy_h = src_h.min(self.height);
        let src_x = yuyv_aligned((src_w - copy_w) / 2);
        let src_y = (src_h - copy_h) / 2;
        let dst_x = yuyv_aligned((self.width - copy_w) / 2);
        let dst_y = (self.height - copy_h) / 2;

        for row in 0..copy_h {
            let src_start = ((src_y + row) * src_w + src_x) * 3;
            let src = &self.ycbcr[src_start..src_start + copy_w * 3];
            let dst_start = ((dst_y + row) * self.width + dst_x) * 2;
            let dst = &mut self.out[dst_start..dst_start + copy_w * 2];
            let (src_pairs, _) = src.as_chunks::<6>();
            let (dst_pairs, _) = dst.as_chunks_mut::<4>();
            for (s, d) in src_pairs.iter().zip(dst_pairs) {
                d[0] = self.luma[s[0] as usize];
                d[1] = self.chroma[(s[1] as usize + s[4] as usize).div_ceil(2)];
                d[2] = self.luma[s[3] as usize];
                d[3] = self.chroma[(s[2] as usize + s[5] as usize).div_ceil(2)];
            }
        }
        Ok(&self.out)
    }

    fn fill_black(&mut self) {
        for px in self.out.as_chunks_mut::<4>().0 {
            *px = BLACK;
        }
        self.last_src_size = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE_8X4: &[u8] = include_bytes!("testdata/white-8x4.jpg");
    const WHITE: [u8; 4] = [235, 128, 235, 128];

    fn pixel(frame: &[u8], width: usize, x: usize, y: usize) -> u8 {
        frame[(y * width + x) * 2]
    }

    #[test]
    fn limited_range_same_size() {
        let mut conv = Converter::new(8, 4);
        let frame = conv.convert(WHITE_8X4).unwrap();
        assert_eq!(frame.len(), 8 * 4 * 2);
        assert!(frame.as_chunks::<4>().0.iter().all(|p| *p == WHITE));
    }

    #[test]
    fn pads_smaller_source_with_black() {
        let mut conv = Converter::new(12, 6);
        let frame = conv.convert(WHITE_8X4).unwrap();
        assert_eq!(pixel(frame, 12, 0, 0), 16);
        assert_eq!(pixel(frame, 12, 1, 2), 16);
        assert_eq!(pixel(frame, 12, 2, 1), 235);
        assert_eq!(pixel(frame, 12, 9, 4), 235);
        assert_eq!(pixel(frame, 12, 10, 4), 16);
        assert_eq!(pixel(frame, 12, 5, 5), 16);
    }

    #[test]
    fn crops_larger_source() {
        let mut conv = Converter::new(4, 2);
        let frame = conv.convert(WHITE_8X4).unwrap();
        assert!(frame.as_chunks::<4>().0.iter().all(|p| *p == WHITE));
    }
}
