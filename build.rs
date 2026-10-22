//! Icône de l'exécutable sous Windows : `assets/archforge.ico` devient un fichier de ressources (.res)
//! passé à l'éditeur de liens MSVC, sans dépendre du compilateur de ressources du SDK Windows.

use std::path::PathBuf;

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;

fn push_entry(out: &mut Vec<u8>, kind: u16, id: u16, data: &[u8]) {
    let header_size: u32 = 32;
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&0x1030u16.to_le_bytes()); // MemoryFlags : moveable, pure, discardable
    out.extend_from_slice(&0u16.to_le_bytes()); // LanguageId : neutre
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    out.extend_from_slice(data);
    while out.len() % 4 != 0 {
        out.push(0);
    }
}

fn ico_to_res(ico: &[u8]) -> Vec<u8> {
    let u16_at = |o: usize| u16::from_le_bytes([ico[o], ico[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes([ico[o], ico[o + 1], ico[o + 2], ico[o + 3]]);
    let count = u16_at(4) as usize;

    // Entrée vide qui ouvre tout fichier .res 32 bits : 32 octets, données 0, type et nom 0.
    let mut res = Vec::new();
    res.extend_from_slice(&[0, 0, 0, 0, 0x20, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0]);
    res.extend_from_slice(&[0; 16]);

    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes());
    group.extend_from_slice(&(count as u16).to_le_bytes());
    for i in 0..count {
        let e = 6 + 16 * i;
        let size = u32_at(e + 8) as usize;
        let offset = u32_at(e + 12) as usize;
        let id = (i + 1) as u16;
        push_entry(&mut res, RT_ICON, id, &ico[offset..offset + size]);
        group.extend_from_slice(&ico[e..e + 8]); // largeur, hauteur, couleurs, réservé, plans, bits
        group.extend_from_slice(&(size as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    push_entry(&mut res, RT_GROUP_ICON, 1, &group);
    res
}

fn main() {
    println!("cargo:rerun-if-changed=assets/archforge.ico");
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows-msvc") {
        return;
    }
    let Ok(ico) = std::fs::read("assets/archforge.ico") else { return };
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("archforge.res");
    std::fs::write(&out, ico_to_res(&ico)).expect("écriture de archforge.res");
    println!("cargo:rustc-link-arg-bins={}", out.display());
}
