//! File thumbnails shared by the Media Browser tiles and the in-app file picker's icon views.
//!
//! One decode budget for both: at most `budget` decodes are started per frame (the caller
//! repaints while the budget lasts), and every answer — including "not decodable" — is cached
//! per path in the egui context, so a folder of audio files never probes twice.

use effectcraft_engine::project::ItemId;

use crate::EffectcraftApp;

/// A decoded thumbnail: the texture and its pixel size (`None` while not decoded, or when the
/// file has no picture to show).
pub type Thumb = Option<(egui::TextureHandle, [u32; 2])>;

/// A thumb cache entry: the answer and the file state it was decoded from.
#[derive(Clone)]
struct ThumbEntry {
    stamp: Option<(u64, Option<std::time::SystemTime>)>,
    thumb: Thumb,
}

/// Thumbnail of a file (cached per path; `None` while not decoded or not decodable). Still
/// images and video decode through the media layer's probe and footage cache, downscaled to at
/// most 128 px on the long side.
///
/// The cache key is the path *and* the file's size and modification time, so a save over that
/// path (or a file that appears where an undecodable one was) is decoded afresh instead of
/// showing the old picture — or the old "no picture" — for the rest of the session. On a changed
/// file the media layer is also told to forget the path: its decoded-frame caches key on the
/// path alone ([`forget`](effectcraft_media::MediaPool::forget)), so a re-decode alone would
/// serve and re-cache the overwritten file's old frames.
pub fn media(app: &EffectcraftApp, ctx: &egui::Context, path: &str, budget: &mut u32) -> Thumb {
    let stamp = std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()));
    let id = egui::Id::unique(("file-thumb", path));
    let saved: Option<ThumbEntry> = ctx.data(|d| d.get_temp::<Option<ThumbEntry>>(id)).unwrap_or(None);
    if let Some(saved) = saved
        && saved.stamp == stamp
    {
        // The same file as the saved answer (decodable or not): no work. A failed decode
        // is a saved answer too — the media pool keeps *why* it failed, and re-probing
        // per frame would only repeat it.
        return saved.thumb.clone();
    }
    if *budget == 0 {
        return None;
    }
    *budget -= 1;
    // The decode below re-reads the file as it is now, so the media layer forgets what it has
    // for the path — its decoders and frames are keyed by the path alone (see
    // [`forget`](effectcraft_media::MediaPool::forget)), and so are the recorded problems, so
    // a file that completes (or appears) where an undecodable one was gets a fresh retry —
    // whatever the last answer on this path was.
    app.session.footage.forget(path);
    let thumb = (|| {
        let imp = app.session.importer.clone()?;
        let f = imp.probe(path).ok()?;
        if !f.has_video {
            return None;
        }
        // A synthetic item id keyed by the path (the frame cache keys on the path).
        let hid = path.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3)) | (1 << 62);
        let img = app.session.footage.frame(ItemId(hid), &f, effectcraft_engine::time::Tick::ZERO)?;
        let step = (img.width.max(img.height) as f32 / 128.0).ceil().max(1.0) as u32;
        let (w, h) = (img.width.div_ceil(step), img.height.div_ceil(step));
        let full = img.to_rgba8();
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let i = (((y * step) * img.width + x * step) * 4) as usize;
                px.extend_from_slice(&full[i..i + 4]);
            }
        }
        let tex = ctx.load_texture(
            format!("file-thumb-{path}"),
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &px),
            egui::TextureOptions::LINEAR,
        );
        Some((tex, [w, h]))
    })();
    ctx.data_mut(|d| d.insert_temp(id, Some(ThumbEntry { stamp, thumb: thumb.clone() })));
    thumb
}
