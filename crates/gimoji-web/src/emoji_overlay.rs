//! DOM overlay that paints emoji glyphs over the wgpu canvas.
//!
//! ratatui-wgpu renders the entire picker (borders, search box, code and
//! description columns, scroll bar, toast) through `rustybuzz` + `raqote`,
//! which need the actual TTF bytes for every glyph they rasterise. Color
//! emoji fonts are large (~1.4 MB for Twemoji COLR) and the COLR raster
//! path in ratatui-wgpu is also lossy. To avoid both, we leave the emoji
//! column blank in the canvas and place absolutely-positioned `<span>`s
//! over it — the browser then renders each emoji with its native text
//! engine and system emoji font (Apple Color Emoji / Segoe UI Emoji /
//! Noto Color Emoji), at native quality, for free.

use gimoji_core::VisibleEmoji;
use ratatui::layout::{Rect, Size};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlCanvasElement, HtmlElement};

pub struct EmojiOverlay {
    root: HtmlElement,
    spans: Vec<HtmlElement>,
    /// Per-span memo of last applied style+text. Avoids re-issuing DOM
    /// writes (which queue layout invalidation in the browser) when the
    /// same emoji stays in the same cell across animation frames.
    last: Vec<SpanState>,
    /// Cached canvas geometry. `None` forces a refresh on the next sync;
    /// see [`Self::invalidate_geometry`].
    geometry: Option<CanvasGeometry>,
    /// Reused scratch buffer to avoid one small allocation per frame.
    scratch: Vec<Item>,
}

/// One absolutely-positioned emoji to draw. The picker uses [`VisibleEmoji`]
/// for rows; the toast's emoji slot is fed in the same form so a single
/// list drives the overlay layout.
struct Item {
    cell: Rect,
    /// Static for picker rows (interned via `&'static str` from the emoji
    /// database) — the toast emoji is a `String` owned by `Toast` and
    /// lives at least until the next sync, but we copy to `&str` form via
    /// a `String` clone to keep the `Item` type uniform and free of
    /// lifetime parameters that would propagate into struct fields.
    text: String,
}

#[derive(Default, PartialEq)]
struct SpanState {
    visible: bool,
    style: String,
    text: String,
}

#[derive(Clone, Copy)]
struct CanvasGeometry {
    cell_w: f64,
    cell_h: f64,
    origin_x: f64,
    origin_y: f64,
    /// Terminal dimensions in cells used to derive `cell_w`/`cell_h`. If
    /// these change, the cached geometry is stale.
    cols: u16,
    rows: u16,
}

impl EmojiOverlay {
    pub fn new(document: &Document) -> Result<Self, JsValue> {
        let root: HtmlElement = document
            .create_element("div")?
            .dyn_into::<HtmlElement>()
            .map_err(|_| JsValue::from_str("overlay root is not HtmlElement"))?;
        root.set_id("gimoji-emoji-overlay");
        document
            .body()
            .ok_or_else(|| JsValue::from_str("no body"))?
            .append_child(&root)?;
        Ok(Self {
            root,
            spans: Vec::new(),
            last: Vec::new(),
            geometry: None,
            scratch: Vec::new(),
        })
    }

    /// Drop the cached canvas geometry so the next `sync` re-reads
    /// `getBoundingClientRect()`. Call after the canvas is resized — at
    /// other times the cached values are valid and re-reading would force
    /// an extra layout reflow per frame.
    pub fn invalidate_geometry(&mut self) {
        self.geometry = None;
    }

    pub fn sync(
        &mut self,
        canvas: &HtmlCanvasElement,
        term: Size,
        rows: &[VisibleEmoji],
        toast: Option<(Rect, &str)>,
    ) {
        // Emoji glyphs render at the full em-square height with no
        // internal padding, unlike Latin text where the cap height is
        // typically ~70% of font-size. Setting font-size = cell_h would
        // make emojis visibly larger than the surrounding monospace
        // text and butt against neighbouring rows. Match the canvas
        // text's visual height instead by scaling the emoji to roughly
        // the cap-height fraction, then let the flex container centre
        // it vertically inside the full cell box.
        const EMOJI_HEIGHT_RATIO: f64 = 0.72;

        let geom = self.geometry(canvas, term);

        // Reuse the scratch buffer across calls. Picker scroll rapidly
        // rewrites the items list every frame; allocating a fresh Vec
        // each time put unnecessary GC pressure on the browser.
        self.scratch.clear();
        self.scratch.reserve(rows.len() + toast.is_some() as usize);
        for ve in rows {
            self.scratch.push(Item {
                cell: ve.cell,
                text: ve.emoji.to_string(),
            });
        }
        if let Some((cell, text)) = toast {
            self.scratch.push(Item {
                cell,
                text: text.to_string(),
            });
        }

        let items_len = self.scratch.len();
        self.ensure_capacity(items_len);

        for (i, item) in self.scratch.iter().enumerate() {
            let left = geom.origin_x + geom.cell_w * item.cell.x as f64;
            let top = geom.origin_y + geom.cell_h * item.cell.y as f64;
            let width = geom.cell_w * item.cell.width as f64;
            let height = geom.cell_h * item.cell.height as f64;
            let font_size = height * EMOJI_HEIGHT_RATIO;
            let style = format!(
                "left:{:.2}px;top:{:.2}px;width:{:.2}px;height:{:.2}px;font-size:{:.2}px;line-height:{:.2}px",
                left, top, width, height, font_size, height,
            );

            let span = &self.spans[i];
            let prev = &mut self.last[i];

            if !prev.visible || prev.style != style {
                let _ = span.set_attribute("style", &style);
                prev.style.clear();
                prev.style.push_str(&style);
            }
            if prev.text != item.text {
                span.set_text_content(Some(&item.text));
                prev.text.clear();
                prev.text.push_str(&item.text);
            }
            prev.visible = true;
        }

        // Hide stale spans (rows that scrolled off, toast that
        // disappeared). Touch the DOM only on the visibility transition,
        // not every frame.
        for (span, prev) in self.spans.iter().zip(self.last.iter_mut()).skip(items_len) {
            if prev.visible {
                let _ = span.style().set_property("display", "none");
                prev.visible = false;
            }
        }
    }

    fn geometry(&mut self, canvas: &HtmlCanvasElement, term: Size) -> CanvasGeometry {
        if let Some(g) = self.geometry {
            if g.cols == term.width && g.rows == term.height {
                return g;
            }
        }
        let rect = canvas.get_bounding_client_rect();
        let cw = term.width.max(1) as f64;
        let ch = term.height.max(1) as f64;
        let g = CanvasGeometry {
            cell_w: rect.width() / cw,
            cell_h: rect.height() / ch,
            origin_x: rect.left(),
            origin_y: rect.top(),
            cols: term.width,
            rows: term.height,
        };
        self.geometry = Some(g);
        g
    }

    fn ensure_capacity(&mut self, n: usize) {
        while self.spans.len() < n {
            let document = self.root.owner_document().expect("overlay detached");
            let element: Element = document.create_element("span").expect("create span");
            let span: HtmlElement = element.dyn_into().expect("span is HtmlElement");
            let _ = span.set_attribute("aria-hidden", "true");
            self.root.append_child(&span).expect("append span");
            self.spans.push(span);
            self.last.push(SpanState::default());
        }
    }
}
