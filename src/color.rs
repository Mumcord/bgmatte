//! Conversions between RGB and BT.601 limited-range YUV, the colour space webcams and video
//! encoders use and the one the pipeline works in.

/// RGB → YUV for one pixel, unrounded.
#[inline]
fn rgb_to_yuv_f32(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (f32::from(r), f32::from(g), f32::from(b));
    (
        16.0 + (65.481 * r + 128.553 * g + 24.966 * b) / 255.0,
        128.0 + (-37.797 * r - 74.203 * g + 112.0 * b) / 255.0,
        128.0 + (112.0 * r - 93.786 * g - 18.214 * b) / 255.0,
    )
}

#[inline]
fn to_u8(value: f32) -> u8 {
    (value + 0.5).clamp(0.0, 255.0) as u8
}

/// YUV → RGB for one pixel.
#[inline]
pub(crate) fn yuv_to_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
    let y = (f32::from(y) - 16.0) * (255.0 / 219.0);
    let u = (f32::from(u) - 128.0) * (255.0 / 224.0);
    let v = (f32::from(v) - 128.0) * (255.0 / 224.0);
    [
        to_u8(y + 1.402 * v),
        to_u8(y - 0.344_136 * u - 0.714_136 * v),
        to_u8(y + 1.772 * u),
    ]
}

/// Converts packed RGB(A) pixels (`bpp` bytes each, RGB first) of `w × h` into NV12 (`y` then
/// interleaved `uv`). Chroma is the mean of each 2×2 block. Each pixel is converted once, in
/// one pass over the block rows; allocates nothing.
pub(crate) fn packed_to_nv12(src: &[u8], bpp: usize, w: usize, h: usize, nv12: &mut [u8]) {
    let (y_plane, uv_plane) = nv12.split_at_mut(w * h);
    for cy in 0..h / 2 {
        let (top, bottom) = y_plane[cy * 2 * w..][..2 * w].split_at_mut(w);
        let uv = &mut uv_plane[cy * w..][..w];
        let src_top = &src[cy * 2 * w * bpp..][..w * bpp];
        let src_bottom = &src[(cy * 2 + 1) * w * bpp..][..w * bpp];
        for cx in 0..w / 2 {
            let (mut su, mut sv) = (0.0, 0.0);
            for (row, out) in [(src_top, &mut *top), (src_bottom, &mut *bottom)] {
                for x in [cx * 2, cx * 2 + 1] {
                    let px = &row[x * bpp..x * bpp + 3];
                    let (y, u, v) = rgb_to_yuv_f32(px[0], px[1], px[2]);
                    out[x] = to_u8(y);
                    su += u;
                    sv += v;
                }
            }
            uv[cx * 2] = to_u8(su * 0.25);
            uv[cx * 2 + 1] = to_u8(sv * 0.25);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb_to_yuv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let (y, u, v) = rgb_to_yuv_f32(r, g, b);
        (to_u8(y), to_u8(u), to_u8(v))
    }

    #[test]
    fn converts_white_and_black_to_limited_range() {
        assert_eq!(rgb_to_yuv(255, 255, 255), (235, 128, 128));
        assert_eq!(rgb_to_yuv(0, 0, 0), (16, 128, 128));
    }

    #[test]
    fn round_trips_within_rounding() {
        for rgb in [[0, 255, 0], [200, 30, 90], [12, 140, 250], [128, 128, 128]] {
            let (y, u, v) = rgb_to_yuv(rgb[0], rgb[1], rgb[2]);
            let back = yuv_to_rgb(y, u, v);
            for (a, b) in rgb.iter().zip(back) {
                assert!(a.abs_diff(b) <= 3, "{rgb:?} came back as {back:?}");
            }
        }
    }

    #[test]
    fn packs_rgba_into_nv12() {
        let (w, h) = (4, 2);
        let rgba: Vec<u8> = (0..w * h).flat_map(|_| [0, 255, 0, 7]).collect();
        let mut nv12 = vec![0u8; w * h * 3 / 2];
        packed_to_nv12(&rgba, 4, w, h, &mut nv12);
        assert!(nv12[..w * h].iter().all(|&y| y == 145));
        assert_eq!(&nv12[w * h..], &[54, 34, 54, 34]);
    }
}
