//! ZIP64 without forcing it (10 §6.2 ZIP-01): the writer switches to ZIP64 records by itself when the entry count
//! reaches 65,535, or an entry is 4 GiB or more, and the reader follows them. The forced ZIP64 paths are covered by the
//! unit tests of `zip.rs`.
//!
//! The 4 GiB case writes and reads 4.3 GB of data. It is `#[ignore]`d in the default test run and run in release:
//! `cargo test -p tt-persist --release --test zip64 -- --include-ignored`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::io::{Read, Write};
use std::path::PathBuf;

use tt_persist::zip::{Limits, Method, ZipArchive, ZipWriter};

const MIMETYPE: &str = "application/x-animpkg+zip";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tt-zip64-{}", tt_persist::fsx::random_token(4)));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// The ZIP64 end-of-central-directory record signature.
fn has_zip64_end(bytes: &[u8]) -> bool {
    bytes.windows(4).rev().take(1 << 16).any(|w| w == [0x50, 0x4b, 0x06, 0x06])
}

#[test]
fn more_than_65535_entries_switch_to_zip64_records() {
    let p = tmp("many.zip");
    let mut w = ZipWriter::new(std::io::BufWriter::new(std::fs::File::create(&p).unwrap()));
    w.add_bytes("mimetype", MIMETYPE.as_bytes(), Method::Stored).unwrap();
    let n = 70_000u32;
    for i in 0..n {
        w.add_bytes(&format!("media/e{i:05}.bin"), &i.to_le_bytes(), Method::Stored).unwrap();
    }
    w.finish().unwrap().flush().unwrap();
    let bytes = std::fs::read(&p).unwrap();
    assert!(has_zip64_end(&bytes), "a ZIP64 end record was written");
    let z = ZipArchive::open(&p, Some(MIMETYPE), Limits::default()).unwrap();
    assert_eq!(z.entries().len(), n as usize + 1);
    assert_eq!(z.read("media/e69999.bin", 4).unwrap(), 69_999u32.to_le_bytes());
    assert_eq!(z.read("media/e00000.bin", 4).unwrap(), 0u32.to_le_bytes());
    // The entry-count limit applies to ZIP64 archives as well (ZIP-06).
    let e = ZipArchive::open(&p, Some(MIMETYPE), Limits { max_entries: 50_000, ..Limits::default() }).unwrap_err();
    assert_eq!(e.details()["reason"], "sizeLimit", "{e:?}");
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

/// A reader of `n` bytes of a repeating pattern.
struct Pattern {
    pos: u64,
    left: u64,
}

impl Read for Pattern {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let k = usize::try_from(self.left.min(buf.len() as u64)).unwrap();
        for (i, b) in buf[..k].iter_mut().enumerate() {
            *b = ((self.pos + i as u64) % 251) as u8;
        }
        self.pos += k as u64;
        self.left -= k as u64;
        Ok(k)
    }
}

#[test]
#[ignore = "writes and reads 4.3 GB; run in release with --include-ignored"]
fn an_entry_over_4_gib_round_trips_with_zip64_sizes() {
    let size: u64 = (1 << 32) + 4096 * 251;
    let p = tmp("big.zip");
    let mut w = ZipWriter::new(std::io::BufWriter::new(std::fs::File::create(&p).unwrap()));
    w.add_bytes("mimetype", MIMETYPE.as_bytes(), Method::Stored).unwrap();
    w.add_bytes("project.json", b"{}\n", Method::Deflate).unwrap();
    w.add_reader("media/big.bin", Pattern { pos: 0, left: size }, size, Method::Deflate).unwrap();
    w.add_bytes("media/after.bin", b"after", Method::Stored).unwrap();
    w.finish().unwrap().flush().unwrap();
    let z = ZipArchive::open(&p, Some(MIMETYPE), Limits::default()).unwrap();
    assert_eq!(z.entries().len(), 4);
    let big = z.entry("media/big.bin").unwrap().clone();
    assert_eq!(big.uncompressed, size);
    assert_eq!(z.read("media/after.bin", 5).unwrap(), b"after");
    // Stream it back and compare with the pattern.
    let mut r = z.reader("media/big.bin", size).unwrap();
    let mut buf = vec![0u8; 251 * 4096];
    let mut total = 0u64;
    loop {
        let k = r.read(&mut buf).unwrap();
        if k == 0 {
            break;
        }
        for (i, b) in buf[..k].iter().enumerate() {
            assert_eq!(*b, ((total + i as u64) % 251) as u8);
        }
        total += k as u64;
    }
    assert_eq!(total, size);
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}
