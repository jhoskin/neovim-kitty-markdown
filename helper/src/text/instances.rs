//! Named instances of variable fonts as faces of their own.
//!
//! fontdb lists one face per font in a file: the default instance of a
//! variable font. CoreText and Blink see every named instance in the
//! `fvar` table as a face with its own PostScript name, and Chrome on
//! macOS draws 400 Han text in `PingFangSC-Regular`, 600 and 700 in
//! `PingFangSC-Semibold`, while fontdb only knows `PingFangSC-Medium`, the
//! default. So the font system adds one face per named instance, with the
//! instance's PostScript name and weight, and shapes it at the instance's
//! axis values.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use rustybuzz::ttf_parser::{self, Tag};
use usvg::fontdb;

/// One named instance: its PostScript name, the CSS weight its `wght`
/// value gives (when the axis uses the CSS scale), and its axis values.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedInstance {
    pub postscript: String,
    pub weight: Option<u16>,
    pub coords: Vec<rustybuzz::Variation>,
}

const WGHT: Tag = Tag::from_bytes(b"wght");
/// The `postScriptNameID` that means the instance has no name.
const NO_NAME: u16 = 0xFFFF;

fn be_u16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]))
}

fn be_u32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *data.get(at)?,
        *data.get(at + 1)?,
        *data.get(at + 2)?,
        *data.get(at + 3)?,
    ]))
}

/// The named instances of `face` that carry a PostScript name, from the
/// `fvar` table (OpenType 1.9, "fvar: InstanceRecord"). ttf-parser reads
/// the axes of `fvar` but not its instance records.
pub fn named_instances(face: &ttf_parser::Face<'_>) -> Vec<NamedInstance> {
    let Some(data) = face.raw_face().table(Tag::from_bytes(b"fvar")) else {
        return Vec::new();
    };
    parse_fvar(data, |id| {
        face.names()
            .into_iter()
            .filter(|name| name.name_id == id && name.is_unicode())
            .find_map(|name| name.to_string())
    })
    .unwrap_or_default()
}

fn parse_fvar(data: &[u8], name: impl Fn(u16) -> Option<String>) -> Option<Vec<NamedInstance>> {
    let axes_offset = usize::from(be_u16(data, 4)?);
    let axis_count = usize::from(be_u16(data, 8)?);
    let axis_size = usize::from(be_u16(data, 10)?);
    let instance_count = usize::from(be_u16(data, 12)?);
    let instance_size = usize::from(be_u16(data, 14)?);
    let mut tags = Vec::with_capacity(axis_count);
    for i in 0..axis_count {
        tags.push(Tag(be_u32(data, axes_offset + i * axis_size)?));
    }
    // The record has a `postScriptNameID` only when it is two bytes longer
    // than the coordinates need.
    let with_name = instance_size >= axis_count * 4 + 6;
    let start = axes_offset + axis_count * axis_size;
    let mut out = Vec::new();
    for i in 0..instance_count {
        let at = start + i * instance_size;
        let coords: Option<Vec<rustybuzz::Variation>> = tags
            .iter()
            .enumerate()
            .map(|(k, &tag)| {
                let fixed = be_u32(data, at + 4 + k * 4)? as i32;
                Some(rustybuzz::Variation {
                    tag,
                    value: fixed as f32 / 65536.0,
                })
            })
            .collect();
        let coords = coords?;
        if !with_name {
            continue;
        }
        let name_id = be_u16(data, at + 4 + axis_count * 4)?;
        if name_id == NO_NAME {
            continue;
        }
        let Some(postscript) = name(name_id) else {
            continue;
        };
        let weight = coords
            .iter()
            .find(|v| v.tag == WGHT)
            .map(|v| v.value)
            .filter(|w| (1.0..=1000.0).contains(w))
            .map(|w| w.round() as u16);
        out.push(NamedInstance {
            postscript,
            weight,
            coords,
        });
    }
    Some(out)
}

/// Adds a face to `db` for every named instance of every variable face in
/// it whose PostScript name the database does not list yet, and returns
/// the new faces with their axis values. Each font file is mapped once.
pub fn register(db: &mut fontdb::Database) -> Vec<(fontdb::ID, Vec<rustybuzz::Variation>)> {
    let mut known: HashSet<String> = db.faces().map(|f| f.post_script_name.clone()).collect();
    let mut groups: Vec<Vec<fontdb::FaceInfo>> = Vec::new();
    let mut by_path: HashMap<PathBuf, usize> = HashMap::new();
    for face in db.faces() {
        let path = match &face.source {
            fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => Some(path),
            fontdb::Source::Binary(_) => None,
        };
        match path.and_then(|p| by_path.get(p)) {
            Some(&i) => groups[i].push(face.clone()),
            None => {
                if let Some(path) = path {
                    by_path.insert(path.clone(), groups.len());
                }
                groups.push(vec![face.clone()]);
            }
        }
    }
    let mut found: Vec<(fontdb::FaceInfo, NamedInstance)> = Vec::new();
    for group in &groups {
        let Some(first) = group.first() else {
            continue;
        };
        db.with_face_data(first.id, |data, _| {
            for base in group {
                if let Ok(face) = ttf_parser::Face::parse(data, base.index)
                    && face.is_variable()
                {
                    found.extend(
                        named_instances(&face)
                            .into_iter()
                            .map(|instance| (base.clone(), instance)),
                    );
                }
            }
        });
    }
    let mut out = Vec::new();
    for (base, instance) in found {
        if !known.insert(instance.postscript.clone()) {
            continue;
        }
        let mut info = base;
        info.post_script_name = instance.postscript;
        if let Some(weight) = instance.weight {
            info.weight = fontdb::Weight(weight);
        }
        let id = db.push_face_info(info);
        out.push((id, instance.coords));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An `fvar` with axes `wght` and `wdth` and two instances, the second
    /// without a PostScript name.
    fn fvar() -> Vec<u8> {
        let mut d = Vec::new();
        for v in [1u16, 0, 16, 2, 2, 20, 2, 14] {
            d.extend_from_slice(&v.to_be_bytes());
        }
        for (tag, min, def, max) in [(b"wght", 100i32, 400, 900), (b"wdth", 50, 100, 200)] {
            d.extend_from_slice(tag);
            for v in [min, def, max] {
                d.extend_from_slice(&(v << 16).to_be_bytes());
            }
            d.extend_from_slice(&0u16.to_be_bytes());
            d.extend_from_slice(&256u16.to_be_bytes());
        }
        for (sub, w, ps) in [(257u16, 600i32, 300u16), (258, 300, NO_NAME)] {
            d.extend_from_slice(&sub.to_be_bytes());
            d.extend_from_slice(&0u16.to_be_bytes());
            d.extend_from_slice(&(w << 16).to_be_bytes());
            d.extend_from_slice(&(100i32 << 16).to_be_bytes());
            d.extend_from_slice(&ps.to_be_bytes());
        }
        d
    }

    #[test]
    fn instance_records_give_name_weight_and_coords() {
        let names = |id: u16| (id == 300).then(|| "Demo-Semibold".to_string());
        let got = parse_fvar(&fvar(), names).expect("parses");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].postscript, "Demo-Semibold");
        assert_eq!(got[0].weight, Some(600));
        assert_eq!(got[0].coords.len(), 2);
        assert_eq!(got[0].coords[1].tag, Tag::from_bytes(b"wdth"));
        assert_eq!(got[0].coords[1].value, 100.0);
    }

    #[test]
    fn a_truncated_table_gives_nothing() {
        assert!(parse_fvar(&fvar()[..20], |_| None).is_none());
    }
}
