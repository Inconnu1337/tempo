pub fn icon_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    let aa = 1.0f32;
    for y in 0..size {
        for x in 0..size {
            let fx = (x as f32 + 0.5) / s;
            let fy = (y as f32 + 0.5) / s;
            let r = 0.22f32;
            let qx = (fx - 0.5).abs() - (0.5 - r) + 0.02;
            let qy = (fy - 0.5).abs() - (0.5 - r) + 0.02;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
            let tile_a = (0.5 - outside * s / aa).clamp(0.0, 1.0);
            if tile_a <= 0.0 {
                continue;
            }
            let t = ((fx + fy) * 0.5).clamp(0.0, 1.0);
            let (r0, g0, b0) = (124.0, 58.0, 237.0);
            let (r1, g1, b1) = (34.0, 211.0, 238.0);
            let mut cr = r0 + (r1 - r0) * t;
            let mut cg = g0 + (g1 - g0) * t;
            let mut cb = b0 + (b1 - b0) * t;

            let dx = fx - 0.5;
            let dy = fy - 0.5;
            let d = (dx * dx + dy * dy).sqrt();
            let ring = ((d - 0.29).abs() - 0.045) * s;
            let mut white = (0.5 - ring / aa).clamp(0.0, 1.0);
            let hand = |ax: f32, ay: f32, bx: f32, by: f32, w: f32| -> f32 {
                let (px_, py_) = (fx - ax, fy - ay);
                let (vx, vy) = (bx - ax, by - ay);
                let h = ((px_ * vx + py_ * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
                let ex = px_ - vx * h;
                let ey = py_ - vy * h;
                let dd = (ex * ex + ey * ey).sqrt() - w;
                (0.5 - dd * s / aa).clamp(0.0, 1.0)
            };
            white = white
                .max(hand(0.5, 0.5, 0.5, 0.31, 0.04))
                .max(hand(0.5, 0.5, 0.64, 0.56, 0.04));
            cr += (255.0 - cr) * white;
            cg += (255.0 - cg) * white;
            cb += (255.0 - cb) * white;

            let i = ((y * size + x) * 4) as usize;
            px[i] = cr as u8;
            px[i + 1] = cg as u8;
            px[i + 2] = cb as u8;
            px[i + 3] = (tile_a * 255.0) as u8;
        }
    }
    px
}

pub fn icon_bmp_payload(size: u32) -> Vec<u8> {
    let rgba = icon_rgba(size);
    let mut v = Vec::new();
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    le32(&mut v, 40);
    le32(&mut v, size);
    le32(&mut v, size * 2);
    le16(&mut v, 1);
    le16(&mut v, 32);
    le32(&mut v, 0);
    let mask_row = size.div_ceil(32) * 4;
    le32(&mut v, size * size * 4 + mask_row * size);
    for _ in 0..4 {
        le32(&mut v, 0);
    }
    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            v.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    v.extend(std::iter::repeat_n(0u8, (mask_row * size) as usize));
    v
}

#[allow(dead_code)]
pub fn ico_file(sizes: &[u32]) -> Vec<u8> {
    let payloads: Vec<Vec<u8>> = sizes.iter().map(|&s| icon_bmp_payload(s)).collect();
    let mut v = vec![0, 0, 1, 0];
    v.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, p) in sizes.iter().zip(&payloads) {
        let b = if *s >= 256 { 0 } else { *s as u8 };
        v.extend_from_slice(&[b, b, 0, 0, 1, 0, 32, 0]);
        v.extend_from_slice(&(p.len() as u32).to_le_bytes());
        v.extend_from_slice(&offset.to_le_bytes());
        offset += p.len() as u32;
    }
    for p in payloads {
        v.extend(p);
    }
    v
}
