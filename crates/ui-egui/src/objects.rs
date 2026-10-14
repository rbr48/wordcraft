//! Pictures, shapes and text boxes on the canvas: click one to select it, drag it to move it,
//! drag a handle to resize it, drag the rotation handle to turn it (Shift: 15° steps), arrow keys
//! nudge a floating one. The frame itself is [`crate::frame`]; changes go through
//! `arrange.bounds` / `arrange.rotation` / `arrange.nudge`, so they're undoable and scriptable like
//! everything else.
//!
//! While dragging, nothing is laid out or rendered: the preview is the object's own pixels taken
//! from its page's cached texture. The drop is one command, one relayout.

use egui::{Color32, Painter, Pos2, Rect, Ui};
use serde_json::json;
use wordcraft_doc::para::{InlineObject, OBJ};
use wordcraft_doc::{PartKind, Pos, StoryRef};
use wordcraft_layout::DocLayout;
use wordcraft_layout::hit::ObjectHit;

use crate::WordApp;
use crate::frame::{self, Drag, Grab};
use crate::theme::Tokens;

/// The band along a text box's border that grabs the box rather than its text, screen points.
const EDGE: f32 = 5.0;
/// How opaque the drag preview of an object's pixels is (0–255).
const PREVIEW_OPACITY: u8 = 170;
/// Arrow-key nudge, points (Ctrl/Alt: fine).
const NUDGE: f32 = 6.0;
const NUDGE_FINE: f32 = 1.0;

/// An object being dragged by its frame.
pub struct ObjectDrag {
    object: ObjectHit,
    drag: Drag,
    /// Smallest size it can be resized to, points.
    min: f32,
    picture: bool,
    initial_crop: Option<[f32; 4]>,
}

/// The selected object: the selection is exactly one picture, shape or text box in the body.
pub fn selected(app: &WordApp) -> Option<(Pos, &InlineObject)> {
    wordcraft_engine::cmd::objects::object_selection(&app.session).filter(|(p, _)| p.story == StoryRef::Body)
}

/// Whether the object under `hit` keeps its place and size: charts and diagrams can be selected
/// and deleted, not moved or resized (they aren't written back on save yet), so they get no
/// handles, no move cursor and no drag.
fn fixed(app: &WordApp, hit: &ObjectHit) -> bool {
    let pos = hit.pos();
    matches!(app.session.doc.para_at(&pos).and_then(|p| p.object_at(pos.off)), Some(InlineObject::Graphic { .. }))
}

/// The story of the selected text box (or the first text box of a selected group).
fn selected_text_box(app: &WordApp) -> Option<u32> {
    match selected(app)? {
        (_, InlineObject::Shape { story: Some(id), .. }) if app.session.doc.parts.get(id).is_some_and(|p| p.kind == PartKind::TextBox) => Some(*id),
        (_, g @ InlineObject::Group { .. }) => {
            g.text_boxes().into_iter().find(|id| app.session.doc.parts.get(id).is_some_and(|p| p.kind == PartKind::TextBox))
        }
        _ => None,
    }
}

/// The object showing a frame, and whether its text is being edited (caret in a text box).
pub fn active(app: &WordApp, layout: &DocLayout) -> Option<(ObjectHit, bool)> {
    let hint = app.session.page_hint;
    if let Some((pos, _)) = selected(app) {
        return layout.object(&pos, hint).map(|o| (o, false));
    }
    match app.session.sel.focus.story {
        StoryRef::Part(id) if crate::canvas::in_text_box(app) => layout.text_box(id, hint).map(|o| (o, true)),
        _ => None,
    }
}

pub(crate) fn screen(pages: &[Rect], scale: f32, page: usize, r: wordcraft_geom::Rect) -> Option<Rect> {
    pages.get(page).map(|p| frame::to_screen(*p, scale, r))
}

/// Pressing, dragging, releasing or double-clicking on an object (or a frame handle). Returns
/// true when it was about an object, so the text doesn't also get it.
pub fn pointer(app: &mut WordApp, ui: &Ui, resp: &egui::Response, pages: &[Rect], layout: &DocLayout, scale: f32, at: Pos2) -> bool {
    if let Some(mut d) = app.canvas.obj_drag.take() {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            return true; // cancelled: dropped without a change
        }
        if ui.input(|i| i.pointer.primary_down()) {
            let shift = ui.input(|i| i.modifiers.shift);
            if let (Grab::Crop(h), Some(c0)) = (d.drag.grab, d.initial_crop) {
                let dx = (at.x - d.drag.press.x) / scale.max(0.01);
                let dy = (at.y - d.drag.press.y) / scale.max(0.01);
                let w = d.object.rect.w.max(1.0);
                let h_size = d.object.rect.h.max(1.0);
                let mut c = c0;
                if h.x < 0 {
                    c[0] = (c0[0] + dx / w).clamp(0.0, 0.45);
                } else if h.x > 0 {
                    c[2] = (c0[2] - dx / w).clamp(0.0, 0.45);
                }
                if h.y < 0 {
                    c[1] = (c0[1] + dy / h_size).clamp(0.0, 0.45);
                } else if h.y > 0 {
                    c[3] = (c0[3] - dy / h_size).clamp(0.0, 0.45);
                }
                app.session.join_next_undo();
                let _ = app.run("picture.crop", json!({"left": c[0], "top": c[1], "right": c[2], "bottom": c[3]}));
                d.drag.moved = true;
            } else if d.drag.grab == Grab::Rotate {
                d.drag.turn_to(at, pages, scale, shift);
            } else {
                // Pictures keep their proportions from a corner unless Shift; shapes only with Shift.
                d.drag.update(at, pages, scale, d.min, d.picture != shift);
            }
            app.canvas.obj_drag = Some(d);
        } else {
            drop(app, &d);
        }
        return true;
    }
    let pressed = ui.input(|i| i.pointer.primary_pressed()) && resp.contains_pointer();
    let multi = resp.double_clicked() || resp.triple_clicked();
    if !(pressed || multi) || crate::canvas::editing_header_footer(app, layout) {
        return false;
    }
    let Some((object, grab)) = grab_at(app, layout, pages, scale, at) else { return false };
    // Shift/Ctrl+click adds an object to the selected ones (or takes it out), for Group.
    let adding = ui.input(|i| i.modifiers.shift || i.modifiers.command);
    if pressed && adding && grab == Grab::Move && selected(app).is_some() {
        let _ = app.run("select.addObject", json!({"pos": object.pos()}));
        return true;
    }
    if pressed {
        let pos = object.pos();
        let end = Pos { off: pos.off + OBJ.len_utf8(), ..pos.clone() };
        let _ = app.run("select.range", json!({"anchor": pos, "focus": end}));
        let (min, picture, crop) = match selected(app) {
            Some((_, InlineObject::Image { crop, .. })) => {
                let m = wordcraft_engine::cmd::objects::min_size(selected(app).map(|x| x.1).unwrap_or(&InlineObject::Image {
                    media: String::new(),
                    w: 10.0,
                    h: 10.0,
                    alt: String::new(),
                    float: Default::default(),
                    crop: [0.0; 4],
                    ole: None,
                }));
                (m, true, Some(*crop))
            }
            Some((_, o)) => (wordcraft_engine::cmd::objects::min_size(o), false, None),
            None => return true,
        };
        // Objects in table cells can be selected and resized, not moved; charts neither.
        if matches!(grab, Grab::Crop(_)) {
            app.canvas.obj_drag =
                Some(ObjectDrag { drag: Drag::new(grab, object.page, object.rect, object.spin.deg, at), object, min, picture, initial_crop: crop });
        } else if !fixed(app, &object) && (grab != Grab::Move || movable(&object)) {
            app.canvas.obj_drag =
                Some(ObjectDrag { drag: Drag::new(grab, object.page, object.rect, object.spin.deg, at), object, min, picture, initial_crop: None });
        }
    }
    true
}

/// What a press at `at` would grab: a handle of the shown frame, else an object.
fn grab_at(app: &WordApp, layout: &DocLayout, pages: &[Rect], scale: f32, at: Pos2) -> Option<(ObjectHit, Grab)> {
    if app.canvas.crop_active
        && let Some((o, _)) = active(app, layout)
    {
        let pos = o.pos();
        if matches!(app.session.doc.para_at(&pos).and_then(|p| p.object_at(pos.off)), Some(InlineObject::Image { .. }))
            && let Some(f) = screen(pages, scale, o.page, o.rect)
            && let Some(h) = frame::handle_at(f, o.spin.deg, at)
        {
            return Some((o, Grab::Crop(h)));
        }
    }
    let handle = active(app, layout).filter(|(o, _)| o.story == StoryRef::Body && !fixed(app, o)).and_then(|(o, _)| {
        let f = screen(pages, scale, o.page, o.rect)?;
        if frame::rotate_handle_at(f, o.spin.deg, at) {
            return Some((o, Grab::Rotate));
        }
        frame::handle_at(f, o.spin.deg, at).map(|h| (o, Grab::Resize(h)))
    });
    handle.or_else(|| {
        let (page, x, y) = crate::canvas::page_at(pages, layout, scale, at)?;
        // Selectable: body objects (table cells too); notes' objects aren't yet.
        layout.object_at(page, x, y, EDGE / scale.max(0.01)).filter(|o| o.story == StoryRef::Body).map(|o| (o, Grab::Move))
    })
}

/// Objects that can be dragged to a new place: those in the body's own paragraphs.
fn movable(o: &ObjectHit) -> bool {
    o.story == StoryRef::Body && o.path.depth() == 0
}

/// Commit a finished drag as one command.
fn drop(app: &mut WordApp, d: &ObjectDrag) {
    if !d.drag.moved || matches!(d.drag.grab, Grab::Crop(_)) {
        return;
    }
    let r = d.drag.rect;
    let params = match d.drag.grab {
        Grab::Rotate => {
            let _ = app.run("arrange.rotation", json!({"degrees": d.drag.deg}));
            return;
        }
        Grab::Move => json!({"page": d.drag.to_page, "x": r.x, "y": r.y}),
        // An inline object stays in the text; a floating one keeps its far edges put.
        Grab::Resize(_) if d.object.floating() && movable(&d.object) => json!({"width": r.w, "height": r.h, "page": d.drag.page, "x": r.x, "y": r.y}),
        Grab::Resize(_) => json!({"width": r.w, "height": r.h}),
        Grab::Crop(_) => return,
    };
    let _ = app.run("arrange.bounds", params);
}

/// Draw the frame of the active object, or the drag preview.
pub fn paint(app: &WordApp, painter: &Painter, t: &Tokens, layout: &DocLayout, pages: &[Rect], scale: f32) {
    if let Some(d) = app.canvas.obj_drag.as_ref().filter(|d| d.drag.moved) {
        let Some(to) = screen(pages, scale, d.drag.to_page, d.drag.rect) else { return };
        if matches!(d.drag.grab, Grab::Crop(_)) {
            frame::paint_crop(painter, t, to);
            return;
        }
        if d.drag.grab == Grab::Rotate {
            frame::paint_turning(painter, t, to, d.drag.deg);
            return;
        }
        // The object's own pixels, from its page's cached texture: moved, or scaled for an
        // unturned picture. Resizing reflows a text box (and redraws a shape), so that's just the
        // new outline.
        let turned = !d.object.spin.is_identity();
        let pixels = (d.picture && !turned) || d.drag.grab == Grab::Move;
        if !pixels {
            frame::paint_outline(painter, t, to, d.drag.deg);
        } else if let (Some(tex), Some(pg)) = (app.canvas.page_texture(d.object.page), layout.pages.get(d.object.page)) {
            // A turned object's pixels fill its rotated bounds, which move with it.
            let r = d.object.bounds();
            let (dx, dy) = (d.drag.rect.x - d.drag.start.x, d.drag.rect.y - d.drag.start.y);
            let moved = wordcraft_geom::Rect::new(r.x + dx, r.y + dy, r.w, r.h);
            let to = if turned { screen(pages, scale, d.drag.to_page, moved).unwrap_or(to) } else { to };
            let (w, h) = (pg.w.max(1.0), pg.h.max(1.0));
            let uv = Rect::from_min_max(egui::pos2(r.x / w, r.y / h), egui::pos2(r.right() / w, r.bottom() / h));
            painter.image(tex.id(), to, uv, Color32::from_white_alpha(PREVIEW_OPACITY));
        }
        frame::paint(painter, t, to, d.drag.deg, false, false, false);
        return;
    }
    // The others selected along with it (Shift+click): their outlines.
    for p in &app.session.also_selected {
        if let Some(o) = layout.object(p, app.session.page_hint)
            && let Some(f) = screen(pages, scale, o.page, o.rect)
        {
            frame::paint(painter, t, f, o.spin.deg, false, false, false);
        }
    }
    if let Some((o, editing)) = active(app, layout)
        && let Some(f) = screen(pages, scale, o.page, o.rect)
    {
        let pos = o.pos();
        let is_pic = matches!(app.session.doc.para_at(&pos).and_then(|p| p.object_at(pos.off)), Some(InlineObject::Image { .. }));
        if app.canvas.crop_active && is_pic {
            frame::paint_crop(painter, t, f);
        } else {
            let handles = o.story == StoryRef::Body && !fixed(app, &o);
            frame::paint(painter, t, f, o.spin.deg, editing, handles, handles);
        }
    }
}

/// The pointer over an object or a handle (or dragging): its cursor.
pub fn cursor(app: &WordApp, layout: &DocLayout, pages: &[Rect], scale: f32, at: Pos2) -> Option<egui::CursorIcon> {
    if let Some(d) = &app.canvas.obj_drag {
        return Some(d.drag.grab.cursor());
    }
    if crate::canvas::editing_header_footer(app, layout) {
        return None;
    }
    grab_at(app, layout, pages, scale, at).map(|(o, g)| if fixed(app, &o) { egui::CursorIcon::Default } else { g.cursor() })
}

/// Keys for a selected object: arrows nudge a floating one; typing or Enter goes into a text box.
/// Returns true if handled.
pub fn key(app: &mut WordApp, key: egui::Key, m: egui::Modifiers) -> bool {
    if selected(app).is_none() {
        return false;
    }
    let step = if m.ctrl || m.alt || m.command { NUDGE_FINE } else { NUDGE };
    let (dx, dy) = match key {
        egui::Key::ArrowLeft => (-step, 0.0),
        egui::Key::ArrowRight => (step, 0.0),
        egui::Key::ArrowUp => (0.0, -step),
        egui::Key::ArrowDown => (0.0, step),
        egui::Key::Enter => return enter_text_box(app),
        _ => return false,
    };
    // Inline objects move with the text: arrows move the caret as usual.
    app.run("arrange.nudge", json!({"dx": dx, "dy": dy})).is_ok()
}

/// With a text box selected, put the caret at the end of its text (typing goes there).
pub fn enter_text_box(app: &mut WordApp) -> bool {
    let Some(id) = selected_text_box(app) else { return false };
    let end = app.session.doc.end_of(StoryRef::Part(id));
    app.run("caret.set", json!({"pos": end})).is_ok()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wordcraft_doc::para::{InlineObject, ShapeKind};
    use wordcraft_doc::{Document, Pos};

    use super::*;

    /// An app whose first paragraph holds `obj`, and where it's laid out.
    fn app_with(obj: InlineObject) -> (WordApp, ObjectHit) {
        let mut app = WordApp::new(wordcraft_engine::Session::new(Document::new()), crate::Services::default());
        app.session.doc.insert_object(&Pos::body(0, 0), obj, &Default::default()).unwrap();
        let hit = app.session.layout().object(&Pos::body(0, 0), 0).expect("laid out");
        (app, hit)
    }

    #[test]
    fn charts_get_no_handles_or_drag_but_shapes_do() {
        let chart =
            InlineObject::Graphic { w: 200.0, h: 100.0, alt: String::new(), float: Default::default(), graphic: Arc::new(Default::default()) };
        let (app, hit) = app_with(chart);
        assert!(fixed(&app, &hit));
        let shape = InlineObject::Shape {
            kind: ShapeKind::Rectangle,
            w: 50.0,
            h: 50.0,
            fill: None,
            stroke: None,
            stroke_width: 1.0,
            float: Default::default(),
            story: None,
            freeform: None,
            effects: Default::default(),
        };
        let (app, hit) = app_with(shape);
        assert!(!fixed(&app, &hit));
    }

    #[test]
    fn picture_crop_toggle_and_handles() {
        let pic =
            InlineObject::Image { media: "test".into(), w: 100.0, h: 80.0, alt: String::new(), float: Default::default(), crop: [0.0; 4], ole: None };
        let (mut app, _hit) = app_with(pic);
        // Select the picture
        let pos = Pos::body(0, 0);
        let end = Pos::body(0, OBJ.len_utf8());
        app.run("select.range", json!({"anchor": pos, "focus": end})).unwrap();

        // Toggle crop mode ON
        let res = app.run("ui.cropToggle", json!({})).unwrap();
        assert_eq!(res["cropActive"], true);
        assert!(app.canvas.crop_active);

        // Toggle crop mode OFF
        let res2 = app.run("ui.cropToggle", json!({})).unwrap();
        assert_eq!(res2["cropActive"], false);
        assert!(!app.canvas.crop_active);
    }
}
