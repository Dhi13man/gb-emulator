use gb_core::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// DMG palette: 4 shades of green (classic Game Boy colors).
const PALETTE: [[u8; 4]; 4] = [
    [0x9B, 0xBC, 0x0F, 0xFF], // Lightest (color 0)
    [0x8B, 0xAC, 0x0F, 0xFF], // Light (color 1)
    [0x30, 0x62, 0x30, 0xFF], // Dark (color 2)
    [0x0F, 0x38, 0x0F, 0xFF], // Darkest (color 3)
];

/// Render the PPU's 2-bit color buffer to an RGBA pixel buffer.
pub fn render(frame_buffer: &[[u8; SCREEN_WIDTH]; SCREEN_HEIGHT], pixels: &mut [u8]) {
    for (y, row) in frame_buffer.iter().enumerate() {
        for (x, &color) in row.iter().enumerate() {
            let idx = (y * SCREEN_WIDTH + x) * 4;
            let rgba = PALETTE[(color & 0x03) as usize];
            pixels[idx] = rgba[0];
            pixels[idx + 1] = rgba[1];
            pixels[idx + 2] = rgba[2];
            pixels[idx + 3] = rgba[3];
        }
    }
}
