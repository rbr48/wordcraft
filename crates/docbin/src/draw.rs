//! OfficeArt floating drawings (`DggInfo`, `PlcSpaMom`, `PlcSpaHdr`, `PlcfTxbxTxt`,
//! `PlcfHdrTxbxTxt`) from the Table and WordDocument streams ([MS-DOC] §2.8.32, §2.9.91,
//! §2.9.172 and [MS-ODRAW]).
//!
//! Word 97-2003 stores floating pictures, shapes, lines, text boxes and grouped letterheads
//! in the `DggInfo` blob (FIB pair 50) and anchors them at `0x08` characters via `PlcSpaMom`
//! (pair 40, main document) and `PlcSpaHdr` (pair 41, headers/footers). Text box stories live
//! in the `ccpTxbx` and `ccpHdrTxbx` subdocuments, indexed by `PlcfTxbxTxt` (pair 56) and
//! `PlcfHdrTxbxTxt` (pair 58).

use std::collections::HashMap;

use wordcraft_doc::para::{Anchor, Float, GroupChild, InlineObject, MAX_GROUP_CHILDREN, ShapeKind, Wrap};
use wordcraft_doc::props::Rgb;
use wordcraft_doc::{Document, PartKind};

use crate::fib::{self, Fib};
use crate::media::Pictures;
use crate::{MAX_PARTS, WalkCtx, story, table, walk};

/// Maximum OfficeArt container recursion depth.
const MAX_DRAW_DEPTH: usize = 16;
/// Maximum shapes parsed from a single document.
const MAX_SHAPES: usize = 10_000;

/// Parsed OfficeArt shape or group from `DggContainer` / `DgContainer`.
#[derive(Clone, Debug)]
enum DrawNode {
    Group {
        spid: u32,
        spgr_rect: [i32; 4],
        child_rect: Option<[i32; 4]>,
        children: Vec<DrawNode>,
    },
    Shape {
        spid: u32,
        inst: u16,
        flip_h: bool,
        flip_v: bool,
        rot: f32,
        child_rect: Option<[i32; 4]>,
        pib: Option<u32>,
        txid: Option<u32>,
        fill_color: Option<Rgb>,
        no_fill: bool,
        line_color: Option<Rgb>,
        line_width: Option<f32>,
        no_line: bool,
        hidden: bool,
        zero_inset_x: bool,
        zero_inset_y: bool,
    },
}

impl DrawNode {
    fn spid(&self) -> u32 {
        match self {
            DrawNode::Group { spid, .. } | DrawNode::Shape { spid, .. } => *spid,
        }
    }

    fn child_rect(&self) -> Option<[i32; 4]> {
        match self {
            DrawNode::Group { child_rect, .. } | DrawNode::Shape { child_rect, .. } => *child_rect,
        }
    }

    /// Compensate for `wordcraft-layout`'s fixed `BOX_INSET_X = 7.2` / `BOX_INSET_Y = 3.6`
    /// when an OfficeArt borderless textbox explicitly specifies `dxTextLeft = 0` / `dyTextTop = 0`.
    fn borderless_txbx_inset_adj(&self) -> (f32, f32, f32) {
        if let DrawNode::Shape { inst, txid, fill_color, no_fill, no_line, zero_inset_x, zero_inset_y, .. } = self
            && (txid.is_some() || *inst == 202)
            && (*no_fill || fill_color.is_none())
            && *no_line
        {
            let dx = if *zero_inset_x { -7.2 } else { 0.0 };
            let dy = if *zero_inset_y { -3.6 } else { 0.0 };
            let dw = if *zero_inset_x { 14.4 } else { 0.0 };
            (dx, dy, dw)
        } else {
            (0.0, 0.0, 0.0)
        }
    }
}

/// Parse all textbox stories (`PlcfTxbxTxt` and `PlcfHdrTxbxTxt`), `DggInfo` shapes and BLIPs,
/// and return sorted `(abs_cp, InlineObject)` anchors for the main document and header subdocuments.
#[allow(clippy::too_many_arguments)]
pub(crate) fn parse_drawings(
    word: &[u8],
    table_stm: &[u8],
    fib: &Fib,
    ctx: &WalkCtx,
    doc: &mut Document,
    pics: &mut Pictures,
    txbx_base: u32,
    hdr_txbx_base: u32,
) -> Vec<(u32, InlineObject)> {
    let Some((dgg_fc, dgg_lcb)) = fib.pair(fib::pair::DGG_INFO).filter(|(_, l)| *l > 0) else {
        return Vec::new();
    };
    let Some(dgg_end) = (dgg_fc as usize).checked_add(dgg_lcb as usize) else {
        return Vec::new();
    };
    let Some(dgg) = table_stm.get(dgg_fc as usize..dgg_end) else {
        return Vec::new();
    };

    // 1. Parse main and header textbox stories into `doc.parts`, keyed by `(is_hdr, 1-based txid)`.
    let mut txbx_parts: HashMap<(bool, u32), u32> = HashMap::new();
    parse_txbx_plc(table_stm, fib, fib::pair::PLCF_TXBX_TXT, txbx_base, fib.ccp.txbx, false, ctx, doc, pics, &mut txbx_parts);
    parse_txbx_plc(table_stm, fib, fib::pair::PLCF_HDR_TXBX_TXT, hdr_txbx_base, fib.ccp.hdr_txbx, true, ctx, doc, pics, &mut txbx_parts);

    // 2. Parse `DggInfo`: first `DggContainer` (0xF000) for `BStoreContainer` (0xF001) BLIPs,
    //    then `OfficeArtWordDrawing` (`dgglbl` + `DgContainer` 0xF002) for shapes/groups.
    let mut pibs: Vec<Option<String>> = Vec::new();
    let mut shapes_by_spid: HashMap<u32, DrawNode> = HashMap::new();
    let mut at = 0usize;

    if let Some((ver_inst, rec_type, rec_len, payload)) = read_record(dgg, at)
        && rec_type == 0xF000
    {
        let _ = ver_inst;
        parse_dgg_container(payload, word, pics, &mut pibs);
        at = at.saturating_add(8).saturating_add(rec_len);
    }

    while at < dgg.len() && shapes_by_spid.len() < MAX_SHAPES {
        // In MS-DOC `OfficeArtContent`, each `DgContainer` (0xF002) is preceded by a 1-byte
        // `dgglbl` (0x00 = main doc, 0x01 = header). Handle both with and without the byte.
        let dg_at = if dgg.get(at + 2..at + 4).is_some_and(|b| u16::from_le_bytes([b[0], b[1]]) == 0xF002) {
            at
        } else if dgg.get(at + 3..at + 5).is_some_and(|b| u16::from_le_bytes([b[0], b[1]]) == 0xF002) {
            at + 1
        } else {
            break;
        };
        let Some((_, rec_type, rec_len, payload)) = read_record(dgg, dg_at) else {
            break;
        };
        if rec_type != 0xF002 {
            break;
        }
        parse_dg_container(payload, &mut shapes_by_spid, 0);
        at = dg_at.saturating_add(8).saturating_add(rec_len);
    }

    if shapes_by_spid.is_empty() {
        return Vec::new();
    }

    // 3. Match `PlcSpaMom` (pair 40, base = 0) and `PlcSpaHdr` (pair 41, base = hdd_base) anchors.
    let hdd_base = fib.ccp.text.saturating_add(fib.ccp.ftn);
    let mut out: Vec<(u32, InlineObject)> = Vec::new();
    parse_spa_plc(table_stm, fib, fib::pair::PLCF_SPA_MOM, 0, false, &shapes_by_spid, &pibs, &txbx_parts, &mut out);
    parse_spa_plc(table_stm, fib, fib::pair::PLCF_SPA_HDR, hdd_base, true, &shapes_by_spid, &pibs, &txbx_parts, &mut out);
    out.sort_by_key(|(cp, _)| *cp);
    out
}

#[allow(clippy::too_many_arguments)]
fn parse_txbx_plc(
    table_stm: &[u8],
    fib: &Fib,
    pair_idx: usize,
    sub_base: u32,
    sub_len: u32,
    is_hdr: bool,
    ctx: &WalkCtx,
    doc: &mut Document,
    pics: &mut Pictures,
    txbx_parts: &mut HashMap<(bool, u32), u32>,
) {
    if sub_len == 0 {
        return;
    }
    let Some((fc, lcb)) = fib.pair(pair_idx).filter(|(_, l)| *l >= 30) else {
        return;
    };
    let Some(end) = (fc as usize).checked_add(lcb as usize) else {
        return;
    };
    let Some(plc) = table_stm.get(fc as usize..end) else {
        return;
    };
    // Each FTXBXS is 22 bytes: lcb = (n + 1) * 4 + n * 22 = 26 * n + 4.
    if (plc.len() - 4) % 26 != 0 {
        return;
    }
    let n = ((plc.len() - 4) / 26).min(2048);
    // The last entry in PlcfTxbxTxt is the sentinel guard; entries 0..n-1 are 1-based txids.
    let count = n.saturating_sub(1);
    let mut kept_to = 0u32;
    for i in 0..count {
        if doc.parts.len() >= MAX_PARTS {
            break;
        }
        let Some(cp_a) = u32_at(plc, i * 4) else { break };
        let Some(cp_b) = u32_at(plc, (i + 1) * 4) else { break };
        // The CPs of a well-formed PLC ascend, so stories are disjoint. A story starting before
        // the end of the last one kept (a hostile table pointing many text boxes at the same text)
        // is dropped: every subdocument CP becomes at most one story.
        if cp_a < kept_to {
            continue;
        }
        // Each textbox story ends with a 1-CP guard paragraph mark separator.
        let story_end = cp_b.saturating_sub(1);
        if story_end <= cp_a {
            continue;
        }
        kept_to = cp_b;
        let Some((abs_a, abs_b)) = story(sub_base, sub_len, (cp_a, story_end)) else {
            continue;
        };
        let blocks = table::assemble(walk(ctx, abs_a, abs_b, pics));
        if !blocks.is_empty() {
            let id = doc.add_part(PartKind::TextBox, blocks);
            txbx_parts.insert((is_hdr, (i + 1) as u32), id);
        }
    }
}

fn parse_dgg_container(payload: &[u8], word: &[u8], pics: &mut Pictures, pibs: &mut Vec<Option<String>>) {
    let mut at = 0usize;
    while let Some((_, rec_type, rec_len, data)) = read_record(payload, at) {
        if rec_type == 0xF001 {
            // BStoreContainer: sequence of BSE (0xF007) or inline BLIP records.
            let mut bat = 0usize;
            while let Some((_, b_type, b_len, b_data)) = read_record(data, bat) {
                if pibs.len() >= 5000 {
                    break;
                }
                if b_type == 0xF007 && b_data.len() >= 36 {
                    let size = u32_at(b_data, 20).unwrap_or(0) as usize;
                    let fo_delay = u32_at(b_data, 28).unwrap_or(0) as usize;
                    let cb_name = b_data.get(33).copied().unwrap_or(0) as usize;
                    let inline_off = 36usize.saturating_add(cb_name);
                    let key = if b_data.len() > inline_off {
                        pics.add_blip(&b_data[inline_off..])
                    } else if size > 0
                        && let Some(end) = fo_delay.checked_add(size)
                        && let Some(blip) = word.get(fo_delay..end)
                    {
                        pics.add_blip(blip)
                    } else {
                        None
                    };
                    pibs.push(key);
                } else if (0xF01A..=0xF029).contains(&b_type) {
                    pibs.push(pics.add_blip(b_data));
                }
                bat = bat.saturating_add(8).saturating_add(b_len);
            }
        }
        at = at.saturating_add(8).saturating_add(rec_len);
    }
}

fn parse_dg_container(payload: &[u8], out: &mut HashMap<u32, DrawNode>, depth: usize) {
    if depth > MAX_DRAW_DEPTH || out.len() >= MAX_SHAPES {
        return;
    }
    let mut at = 0usize;
    while let Some((_, rec_type, rec_len, data)) = read_record(payload, at) {
        match rec_type {
            0xF002 => parse_dg_container(data, out, depth + 1),
            0xF003 => {
                if let Some(group) = parse_spgr_container(data, out, depth + 1)
                    && out.len() < MAX_SHAPES
                {
                    out.insert(group.spid(), group);
                }
            }
            0xF004 => {
                if let Some((node, _)) = parse_sp_container(data)
                    && out.len() < MAX_SHAPES
                {
                    out.insert(node.spid(), node);
                }
            }
            _ => {}
        }
        at = at.saturating_add(8).saturating_add(rec_len);
    }
}

fn parse_spgr_container(payload: &[u8], out: &mut HashMap<u32, DrawNode>, depth: usize) -> Option<DrawNode> {
    if depth > MAX_DRAW_DEPTH {
        return None;
    }
    let mut at = 0usize;
    let mut group_spid: Option<u32> = None;
    let mut spgr_rect: [i32; 4] = [0, 0, 1000, 1000];
    let mut group_child_rect: Option<[i32; 4]> = None;
    let mut children: Vec<DrawNode> = Vec::new();
    let mut first_sp = true;

    while let Some((_, rec_type, rec_len, data)) = read_record(payload, at) {
        match rec_type {
            0xF004 => {
                if first_sp {
                    first_sp = false;
                    if let Some((node, spgr)) = parse_sp_container(data) {
                        group_spid = Some(node.spid());
                        if let Some(r) = spgr {
                            spgr_rect = r;
                        }
                        group_child_rect = node.child_rect();
                    }
                } else if let Some((node, _)) = parse_sp_container(data) {
                    if out.len() < MAX_SHAPES {
                        out.insert(node.spid(), node.clone());
                    }
                    if children.len() < MAX_GROUP_CHILDREN {
                        children.push(node);
                    }
                }
            }
            0xF003 => {
                if let Some(sub) = parse_spgr_container(data, out, depth + 1) {
                    if out.len() < MAX_SHAPES {
                        out.insert(sub.spid(), sub.clone());
                    }
                    if children.len() < MAX_GROUP_CHILDREN {
                        children.push(sub);
                    }
                }
            }
            _ => {}
        }
        at = at.saturating_add(8).saturating_add(rec_len);
    }

    let spid = group_spid?;
    Some(DrawNode::Group { spid, spgr_rect, child_rect: group_child_rect, children })
}

fn parse_sp_container(payload: &[u8]) -> Option<(DrawNode, Option<[i32; 4]>)> {
    let mut spid: Option<u32> = None;
    let mut inst: u16 = 0;
    let mut flip_h = false;
    let mut flip_v = false;
    let mut rot = 0.0f32;
    let mut spgr_rect: Option<[i32; 4]> = None;
    let mut child_rect: Option<[i32; 4]> = None;
    let mut pib: Option<u32> = None;
    let mut txid: Option<u32> = None;
    let mut fill_color: Option<Rgb> = None;
    let mut no_fill = false;
    let mut line_color: Option<Rgb> = None;
    let mut line_width: Option<f32> = None;
    let mut no_line = false;
    let mut hidden = false;
    let mut zero_inset_x = false;
    let mut zero_inset_y = false;

    let mut at = 0usize;
    while let Some((ver_inst, rec_type, rec_len, data)) = read_record(payload, at) {
        match rec_type {
            0xF009 if data.len() >= 16 => {
                // OfficeArtFSPGR: coordinate system of a group shape.
                spgr_rect = rect_i32(data);
            }
            0xF00A if data.len() >= 8 => {
                // OfficeArtFSP: shape id and flags.
                inst = ver_inst >> 4;
                spid = u32_at(data, 0);
                let flags = u32_at(data, 4).unwrap_or(0);
                flip_h = (flags & 0x40) != 0;
                flip_v = (flags & 0x80) != 0;
            }
            0xF00F if data.len() >= 16 => {
                // OfficeArtChildAnchor: coordinates within parent group.
                child_rect = rect_i32(data);
            }
            0xF00D if data.len() >= 4 => {
                // msofbtClientTextbox: high 16 bits hold the 1-based textbox index.
                if let Some(v) = u32_at(data, 0) {
                    let idx = if v >= 0x1_0000 { v >> 16 } else { v };
                    if idx > 0 {
                        txid = Some(idx);
                    }
                }
            }
            0xF00B | 0xF121 | 0xF122 => {
                // OfficeArtFOPT / SecondaryFOPT / TertiaryFOPT.
                let n_props = (ver_inst >> 4) as usize;
                for i in 0..n_props {
                    let Some(entry) = data.get(i * 6..i * 6 + 6) else { break };
                    let pid = u16::from_le_bytes([entry[0], entry[1]]) & 0x3FFF;
                    let op = u32::from_le_bytes([entry[2], entry[3], entry[4], entry[5]]);
                    match pid {
                        0x0004 => {
                            rot = ((op as i32) as f32 / 65536.0).rem_euclid(360.0);
                        }
                        0x0080 if txid.is_none() && op > 0 => {
                            let idx = if op >= 0x1_0000 { op >> 16 } else { op };
                            if idx > 0 {
                                txid = Some(idx);
                            }
                        }
                        0x0081 if op == 0 => {
                            zero_inset_x = true;
                        }
                        0x0082 if op == 0 => {
                            zero_inset_y = true;
                        }
                        0x0104 if op > 0 => {
                            pib = Some(op);
                        }
                        0x0181 => {
                            if let Some(c) = mso_color(op) {
                                fill_color = Some(c);
                            }
                        }
                        0x01BF if (op & 0x0010_0000) != 0 && (op & 0x0000_0010) == 0 => {
                            no_fill = true;
                        }
                        0x01C0 => {
                            if let Some(c) = mso_color(op) {
                                line_color = Some(c);
                            }
                        }
                        0x01CB => {
                            // Line width in EMUs (1 pt = 12700 EMU).
                            line_width = Some((op as f32 / 12700.0).clamp(0.25, 72.0));
                        }
                        0x01FF if (op & 0x0008_0000) != 0 && (op & 0x0000_0008) == 0 => {
                            no_line = true;
                        }
                        0x03BF if (op & 0x0002_0002) == 0x0002_0002 => {
                            hidden = true;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        at = at.saturating_add(8).saturating_add(rec_len);
    }

    let spid = spid?;
    Some((
        DrawNode::Shape {
            spid,
            inst,
            flip_h,
            flip_v,
            rot,
            child_rect,
            pib,
            txid,
            fill_color,
            no_fill,
            line_color,
            line_width,
            no_line,
            hidden,
            zero_inset_x,
            zero_inset_y,
        },
        spgr_rect,
    ))
}

#[allow(clippy::too_many_arguments)]
fn parse_spa_plc(
    table_stm: &[u8],
    fib: &Fib,
    pair_idx: usize,
    cp_base: u32,
    is_hdr: bool,
    shapes: &HashMap<u32, DrawNode>,
    pibs: &[Option<String>],
    txbx_parts: &HashMap<(bool, u32), u32>,
    out: &mut Vec<(u32, InlineObject)>,
) {
    let Some((fc, lcb)) = fib.pair(pair_idx).filter(|(_, l)| *l >= 30) else {
        return;
    };
    let Some(end) = (fc as usize).checked_add(lcb as usize) else {
        return;
    };
    let Some(plc) = table_stm.get(fc as usize..end) else {
        return;
    };
    // Each FSPAP (Spa) is 26 bytes: lcb = (n + 1) * 4 + n * 26 = 30 * n + 4.
    if (plc.len() - 4) % 30 != 0 {
        return;
    }
    let n = ((plc.len() - 4) / 30).min(2048);
    let data_base = (n + 1) * 4;
    for i in 0..n {
        let Some(rel_cp) = u32_at(plc, i * 4) else { break };
        let Some(abs_cp) = cp_base.checked_add(rel_cp) else { continue };
        let Some(spa) = plc.get(data_base + i * 26..data_base + (i + 1) * 26) else {
            break;
        };
        let Some(spid) = u32_at(spa, 0) else { continue };
        let xa_left = i32_at(spa, 4).unwrap_or(0);
        let ya_top = i32_at(spa, 8).unwrap_or(0);
        let xa_right = i32_at(spa, 12).unwrap_or(0);
        let ya_bottom = i32_at(spa, 16).unwrap_or(0);
        let flags = u16::from_le_bytes([spa[20], spa[21]]);

        let bx = (flags >> 1) & 0x03;
        let by = (flags >> 3) & 0x03;
        let wr = (flags >> 5) & 0x0F;
        let below_text = (flags & 0x4000) != 0;

        let h_rel = match bx {
            0 => Anchor::Margin,
            1 => Anchor::Page,
            _ => Anchor::Column,
        };
        let v_rel = match by {
            0 => Anchor::Margin,
            1 => Anchor::Page,
            _ => Anchor::Paragraph,
        };
        let wrap = match wr {
            1 => Wrap::TopAndBottom,
            2 => Wrap::Square,
            4 => Wrap::Tight,
            5 => Wrap::Through,
            _ if below_text => Wrap::BehindText,
            _ => Wrap::InFrontOfText,
        };

        let x = (xa_left as f32 / 20.0).clamp(-2000.0, 2000.0);
        let y = (ya_top as f32 / 20.0).clamp(-2000.0, 2000.0);
        let w = (xa_right.saturating_sub(xa_left) as f32 / 20.0).clamp(1.0, 2000.0);
        let h = (ya_bottom.saturating_sub(ya_top) as f32 / 20.0).clamp(1.0, 2000.0);
        let float = Float { wrap, h_rel, v_rel, x, y, ..Default::default() };

        let Some(node) = shapes.get(&spid) else { continue };
        if let Some(obj) = build_inline_object(node, w, h, float, is_hdr, pibs, txbx_parts, 0) {
            out.push((abs_cp, obj));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_inline_object(
    node: &DrawNode,
    w: f32,
    h: f32,
    mut float: Float,
    is_hdr: bool,
    pibs: &[Option<String>],
    txbx_parts: &HashMap<(bool, u32), u32>,
    depth: usize,
) -> Option<InlineObject> {
    if depth > MAX_DRAW_DEPTH {
        return None;
    }
    match node {
        DrawNode::Group { spgr_rect, children, .. } => {
            let [gl, gt, gr, gb] = *spgr_rect;
            let ch_w = (gr.saturating_sub(gl) as f32 / 20.0).max(1.0);
            let ch_h = (gb.saturating_sub(gt) as f32 / 20.0).max(1.0);
            let mut group_children = Vec::new();
            for ch in children {
                let Some([cl, ct, cr, cb]) = ch.child_rect() else {
                    continue;
                };
                let cx = cl.saturating_sub(gl) as f32 / 20.0;
                let cy = ct.saturating_sub(gt) as f32 / 20.0;
                let cw = (cr.saturating_sub(cl) as f32 / 20.0).max(0.5);
                let ch_height = (cb.saturating_sub(ct) as f32 / 20.0).max(0.5);
                // Flatten nested groups into the parent group's coordinate space so
                // `InlineObject::group_rects` renders all leaf members.
                if let DrawNode::Group { spgr_rect: sub_spgr, children: sub_children, .. } = ch {
                    let [sgl, sgt, sgr, sgb] = *sub_spgr;
                    let sw = (sgr.saturating_sub(sgl) as f32).max(1.0);
                    let sh = (sgb.saturating_sub(sgt) as f32).max(1.0);
                    for sub in sub_children {
                        let Some([scl, sct, scr, scb]) = sub.child_rect() else {
                            continue;
                        };
                        let (dx, dy, dw) = sub.borderless_txbx_inset_adj();
                        let scx = cx + (scl.saturating_sub(sgl) as f32 / sw) * cw + dx;
                        let scy = cy + (sct.saturating_sub(sgt) as f32 / sh) * ch_height + dy;
                        let scw = (((scr.saturating_sub(scl) as f32 / sw) * cw) + dw).max(0.5);
                        let sch = ((scb.saturating_sub(sct) as f32 / sh) * ch_height).max(0.5);
                        if let Some(obj) = build_inline_object(sub, scw, sch, Float::default(), is_hdr, pibs, txbx_parts, depth + 1)
                            && group_children.len() < MAX_GROUP_CHILDREN
                        {
                            group_children.push(GroupChild { x: scx, y: scy, obj });
                        }
                    }
                    continue;
                }
                let (dx, dy, dw) = ch.borderless_txbx_inset_adj();
                if let Some(obj) = build_inline_object(ch, cw + dw, ch_height, Float::default(), is_hdr, pibs, txbx_parts, depth + 1)
                    && group_children.len() < MAX_GROUP_CHILDREN
                {
                    group_children.push(GroupChild { x: cx + dx, y: cy + dy, obj });
                }
            }
            if group_children.is_empty() {
                return None;
            }
            Some(InlineObject::Group { w, h, float, ch_w, ch_h, children: group_children, is_canvas: false })
        }
        DrawNode::Shape { inst, flip_h, flip_v, rot, pib, txid, fill_color, no_fill, line_color, line_width, no_line, hidden, .. } => {
            if *hidden {
                return None;
            }
            float.flip_h = *flip_h;
            float.flip_v = *flip_v;
            float.rot = *rot;

            // Picture frame with a valid BLIP reference.
            if let Some(idx) = *pib
                && let Some(Some(media)) = pibs.get(idx.saturating_sub(1) as usize)
            {
                return Some(InlineObject::Image { media: media.clone(), w, h, alt: String::new(), float, crop: [0.0; 4], ole: None });
            }

            let story = txid.and_then(|id| txbx_parts.get(&(is_hdr, id)).or_else(|| txbx_parts.get(&(false, id))).copied());

            let kind = if story.is_some() || *inst == 202 {
                ShapeKind::TextBox
            } else {
                match *inst {
                    2 => ShapeKind::RoundedRectangle,
                    3 => ShapeKind::Ellipse,
                    4 => ShapeKind::Diamond,
                    5 => ShapeKind::Triangle,
                    6 => ShapeKind::RightTriangle,
                    20 | 32..=40 => ShapeKind::Line,
                    _ => ShapeKind::Rectangle,
                }
            };

            let fill = if *no_fill || kind == ShapeKind::Line { None } else { *fill_color };
            let stroke = if *no_line { None } else { Some(line_color.unwrap_or(Rgb::BLACK)) };
            let stroke_width = if *no_line { 0.0 } else { line_width.unwrap_or(0.75) };

            let mut adj_w = w;
            if depth == 0 {
                let (dx, dy, dw) = node.borderless_txbx_inset_adj();
                float.x += dx;
                float.y += dy;
                adj_w += dw;
            }

            // Borderless transparent text boxes in Word letterheads often size their frame
            // tightly to the original font metrics; give a small vertical cushion so
            // `fit_box` never clips the bottom line.
            let adj_h = if kind == ShapeKind::TextBox && fill.is_none() && stroke.is_none() {
                h + 14.0
            } else if kind == ShapeKind::Line {
                // A horizontal line in OfficeArt often has cb - ct = 1 twip (0.05 pt);
                // keep it near 0 so `display::shape_items` draws a flat horizontal stroke.
                let raw_h = node.child_rect().map(|[_, ct, _, cb]| cb.saturating_sub(ct) as f32 / 20.0).unwrap_or(h);
                if raw_h <= 0.5 { 0.0 } else { h }
            } else {
                h
            };

            Some(InlineObject::Shape {
                kind,
                w: adj_w,
                h: adj_h,
                fill,
                stroke,
                stroke_width,
                float,
                story,
                freeform: None,
                effects: Default::default(),
            })
        }
    }
}

fn read_record(buf: &[u8], at: usize) -> Option<(u16, u16, usize, &[u8])> {
    let hdr = buf.get(at..at.checked_add(8)?)?;
    let ver_inst = u16::from_le_bytes([hdr[0], hdr[1]]);
    let rec_type = u16::from_le_bytes([hdr[2], hdr[3]]);
    let rec_len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
    let end = at.checked_add(8)?.checked_add(rec_len)?.min(buf.len());
    let payload = buf.get(at + 8..end)?;
    Some((ver_inst, rec_type, payload.len(), payload))
}

fn u32_at(buf: &[u8], at: usize) -> Option<u32> {
    let b = buf.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn i32_at(buf: &[u8], at: usize) -> Option<i32> {
    u32_at(buf, at).map(|v| v as i32)
}

fn rect_i32(buf: &[u8]) -> Option<[i32; 4]> {
    Some([i32_at(buf, 0)?, i32_at(buf, 4)?, i32_at(buf, 8)?, i32_at(buf, 12)?])
}

fn mso_color(op: u32) -> Option<Rgb> {
    let flag = (op >> 24) as u8;
    // 0x00 or 0x02 = explicit RGB color; 0x08 = scheme/palette index (skip to avoid wrong colors).
    if flag == 0x00 || flag == 0x02 { Some(Rgb((op & 0xFF) as u8, ((op >> 8) & 0xFF) as u8, ((op >> 16) & 0xFF) as u8)) } else { None }
}
