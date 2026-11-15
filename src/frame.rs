//! Frames handed to the effects: a mutable pixel buffer with its size and format.

/// Pixel layout of a [`Frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// NV12: the full-resolution Y plane followed by the half-resolution interleaved U/V plane
    /// (BT.601 limited range). Processed in place without conversion.
    Nv12,
    /// Packed 8-bit RGB, three bytes per pixel.
    Rgb,
    /// Packed 8-bit RGBA, four bytes per pixel. The alpha channel is left untouched.
    Rgba,
}

impl PixelFormat {
    /// Bytes a tightly packed frame of `width × height` needs in this format.
    pub fn frame_len(self, width: u32, height: u32) -> usize {
        let pixels = width as usize * height as usize;
        match self {
            Self::Nv12 => pixels * 3 / 2,
            Self::Rgb => pixels * 3,
            Self::Rgba => pixels * 4,
        }
    }

    /// Bytes per pixel of the packed formats.
    pub(crate) fn packed_bytes(self) -> Option<usize> {
        match self {
            Self::Nv12 => None,
            Self::Rgb => Some(3),
            Self::Rgba => Some(4),
        }
    }
}

/// One picture to process in place. Rows are tightly packed (no padding at the row ends).
#[derive(Debug)]
pub struct Frame<'a> {
    data: &'a mut [u8],
    width: u32,
    height: u32,
    format: PixelFormat,
}

impl<'a> Frame<'a> {
    /// A frame of the given format. The buffer may be longer than needed; extra bytes are
    /// ignored. A shorter buffer is reported as [`Error::FrameSize`](crate::Error::FrameSize)
    /// when the frame is processed.
    pub fn new(data: &'a mut [u8], width: u32, height: u32, format: PixelFormat) -> Self {
        Self {
            data,
            width,
            height,
            format,
        }
    }

    /// An NV12 frame.
    pub fn nv12(data: &'a mut [u8], width: u32, height: u32) -> Self {
        Self::new(data, width, height, PixelFormat::Nv12)
    }

    /// A packed RGB frame.
    pub fn rgb(data: &'a mut [u8], width: u32, height: u32) -> Self {
        Self::new(data, width, height, PixelFormat::Rgb)
    }

    /// A packed RGBA frame.
    pub fn rgba(data: &'a mut [u8], width: u32, height: u32) -> Self {
        Self::new(data, width, height, PixelFormat::Rgba)
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Pixel layout.
    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// The pixel bytes, exactly as long as the format needs, or `None` when the buffer is too
    /// short.
    pub(crate) fn pixels(&mut self) -> Option<&mut [u8]> {
        let len = self.format.frame_len(self.width, self.height);
        self.data.get_mut(..len)
    }
}
