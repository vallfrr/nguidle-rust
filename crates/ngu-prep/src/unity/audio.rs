//! Decodes the game's sound effects: FMOD sound banks (FSB5) holding Vorbis.
//!
//! FMOD leaves the Vorbis setup header out of the bank and refers to it by
//! CRC-32. The game ships those headers inside its own `UnityPlayer.dll`, so
//! they are looked up there, in the player's own copy of the game.

use lewton::audio::{read_audio_packet, PreviousWindowRight};
use lewton::header::{read_header_ident, read_header_setup};

type Result<T> = std::result::Result<T, String>;

const FREQUENCIES: [u32; 9] = [4000, 8000, 11000, 11025, 16000, 22050, 24000, 32000, 44100];
const MAX_SETUP: usize = 16 * 1024;
/// FMOD's Vorbis uses these two window sizes (as powers of two).
const BLOCK_SIZES: (u8, u8) = (8, 11);

fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *entry = c;
    }
    table
}

/// The setup header with this CRC-32, found in the player library. Headers
/// start with `05 "vorbis"` and a codebook count, then the first codebook.
pub fn find_setup(player: &[u8], crc: u32) -> Option<&[u8]> {
    let table = crc_table();
    let marks = player
        .windows(3)
        .enumerate()
        .filter(|(_, w)| *w == b"BCV")
        .map(|(i, _)| i);
    for mark in marks {
        let Some(start) = mark.checked_sub(8) else {
            continue;
        };
        if player[start..start + 7] != *b"\x05vorbis" {
            continue;
        }
        let mut state = 0xffff_ffffu32;
        for (offset, byte) in player[start..].iter().take(MAX_SETUP).enumerate() {
            state = table[((state ^ u32::from(*byte)) & 0xff) as usize] ^ (state >> 8);
            if !state == crc {
                return Some(&player[start..=start + offset]);
            }
        }
    }
    None
}

fn le32(bytes: &[u8], at: usize) -> Result<u32> {
    let slice = bytes.get(at..at + 4).ok_or("Truncated sound bank")?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()))
}

/// Decodes a one-sound FSB5 Vorbis bank to 16-bit PCM and returns a WAV file.
pub fn bank_to_wav(bank: &[u8], player: &[u8]) -> Result<Vec<u8>> {
    if bank.get(..4) != Some(b"FSB5") {
        return Err("Not an FMOD sound bank.".into());
    }
    let (count, header_size, name_size, mode) = (
        le32(bank, 8)?,
        le32(bank, 12)? as usize,
        le32(bank, 16)? as usize,
        le32(bank, 24)?,
    );
    if count != 1 || mode != 15 {
        return Err("Only single Vorbis sounds are supported.".into());
    }
    let sample = u64::from_le_bytes(
        bank.get(60..68)
            .ok_or("Truncated sound bank")?
            .try_into()
            .unwrap(),
    );
    let rate = *FREQUENCIES
        .get(((sample >> 1) & 0xf) as usize - 1)
        .ok_or("Unknown sample rate")?;
    let channels = (((sample >> 5) & 1) + 1) as u8;
    let frames = (sample >> 34) as usize;
    let mut at = 68;
    let mut crc = None;
    let mut more = sample & 1 != 0;
    while more {
        let chunk = le32(bank, at)?;
        let (size, kind) = (((chunk >> 1) & 0xff_ffff) as usize, chunk >> 25);
        if kind == 11 {
            crc = Some(le32(bank, at + 4)?);
        }
        at += 4 + size;
        more = chunk & 1 != 0;
    }
    let crc = crc.ok_or("The sound bank has no Vorbis information.")?;
    let setup = find_setup(player, crc)
        .ok_or("The Vorbis setup for this sound was not found in the game.")?;

    let mut ident = b"\x01vorbis".to_vec();
    ident.extend(0u32.to_le_bytes());
    ident.push(channels);
    ident.extend(rate.to_le_bytes());
    ident.extend([0; 12]);
    ident.push(BLOCK_SIZES.1 << 4 | BLOCK_SIZES.0);
    ident.push(1);
    let ident = read_header_ident(&ident).map_err(|e| format!("{e:?}"))?;
    let setup = read_header_setup(setup, channels, (BLOCK_SIZES.0, BLOCK_SIZES.1))
        .map_err(|e| format!("{e:?}"))?;

    let data = bank
        .get(60 + header_size + name_size..)
        .ok_or("Truncated sound bank")?;
    let mut window = PreviousWindowRight::new();
    let mut pcm: Vec<i16> = Vec::new();
    let mut at = 0;
    while at + 2 <= data.len() {
        let length = u16::from_le_bytes([data[at], data[at + 1]]) as usize;
        if length == 0 {
            break;
        }
        let packet = data
            .get(at + 2..at + 2 + length)
            .ok_or("Truncated sound data")?;
        at += 2 + length;
        let decoded =
            read_audio_packet(&ident, &setup, packet, &mut window).map_err(|e| format!("{e:?}"))?;
        for frame in 0..decoded.first().map_or(0, Vec::len) {
            for channel in &decoded {
                pcm.push(channel[frame]);
            }
        }
    }
    pcm.truncate(frames * usize::from(channels));
    Ok(wav(&pcm, u16::from(channels), rate))
}

fn wav(pcm: &[i16], channels: u16, rate: u32) -> Vec<u8> {
    let size = (pcm.len() * 2) as u32;
    let mut out = b"RIFF".to_vec();
    out.extend((36 + size).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(channels.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend((rate * u32::from(channels) * 2).to_le_bytes());
    out.extend((channels * 2).to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(size.to_le_bytes());
    out.extend(pcm.iter().flat_map(|s| s.to_le_bytes()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_describes_the_samples() {
        let bytes = wav(&[1, -1, 2, -2], 2, 44100);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 44100);
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 8);
        assert_eq!(bytes.len(), 44 + 8);
    }

    #[test]
    fn setup_is_found_by_its_crc() {
        let mut player = vec![0u8; 20];
        let setup = b"\x05vorbis\x00BCV\x01\x00\x08\x00\x00\x00\x00\x00";
        player.extend(setup);
        player.extend([9; 10]);
        let table = crc_table();
        let crc = !setup.iter().fold(0xffff_ffffu32, |s, b| {
            table[((s ^ u32::from(*b)) & 0xff) as usize] ^ (s >> 8)
        });
        assert_eq!(find_setup(&player, crc), Some(&setup[..]));
        assert_eq!(find_setup(&player, crc ^ 1), None);
    }

    #[test]
    fn other_files_are_refused() {
        assert!(bank_to_wav(b"RIFF....", &[]).is_err());
    }
}
