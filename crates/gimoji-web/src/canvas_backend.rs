//! Canvas 2D [`Backend`] for the picker.
//!
//! The whole grid is painted into a single `<canvas>` element. Compared to
//! the previous DOM-based backend (one `<span>` per cell), this gives us:
//!
//! * **Bounded DOM** — one element instead of `cols × rows` spans, so the
//!   tab's resident memory doesn't climb into hundreds of MB during heavy
//!   scrolling.
//! * **No inline-style strings** — each redraw is `fillRect` + `fillText`
//!   calls, so we don't churn the wasm heap with `String` allocations on
//!   every frame. Linear memory never shrinks, so allocation traffic is the
//!   dominant input to the high-water mark.
//! * **Per-glyph font fallback** — the canvas font stack covers Latin,
//!   box-drawing and colour-emoji fonts; the browser picks per codepoint,
//!   so wide emojis render without the `width: 1ch` CSS hack the DOM
//!   backend needed.
//!
//! Ratatui's [`Buffer::diff`] is "well-formed" (see ratatui-core docs): it
//! never reports the trailing column of a double-width cell on its own. So
//! when this backend sees a cell with display width 2 it paints both
//! columns in one go, and the next column's continuation cell never
//! reaches [`Backend::draw`].

use std::io::{Error as IoError, Result as IoResult};

use ratatui::{
    backend::{Backend, ClearType, WindowSize},
    buffer::Cell,
    layout::{Position, Rect, Size},
    style::{Color, Modifier},
};
use unicode_width::UnicodeWidthStr;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement};

/// Font size for grid cells, in CSS pixels.
const FONT_SIZE_PX: f64 = 16.0;
/// Multiplier from font size to cell height. Tightens line spacing enough
/// that ratatui's box-drawing characters meet at corners.
const LINE_HEIGHT_FACTOR: f64 = 1.2;
/// Font stack used for the canvas context. Latin monospace first so
/// box-drawing glyphs line up, then platform colour-emoji fonts as fallback
/// — canvas 2D does per-codepoint fallback exactly like CSS.
const FONT_FAMILIES: &str = "ui-monospace, \"JetBrains Mono\", \"SF Mono\", \
    Menlo, Consolas, monospace, \"Apple Color Emoji\", \"Segoe UI Emoji\", \
    \"Noto Color Emoji\", \"Twemoji Mozilla\", \"EmojiOne Color\", emoji";

/// Live geometry of the grid, including the measured cell pixel size in CSS
/// pixels. Exposed to the surrounding code (e.g. `pixel_to_cell`) via
/// [`CanvasBackend::geometry`] so the click handler doesn't have to re-query
/// the DOM and re-measure.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub cell_w: f64,
    pub cell_h: f64,
    pub cols: u16,
    pub rows: u16,
}

pub struct CanvasBackend {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    size: Size,
    cell_w: f64,
    cell_h: f64,
    /// Shadow of the most recently painted cells, in row-major order. The
    /// next [`Backend::draw`] compares each incoming cell to this and skips
    /// the paint when nothing changed — ratatui already diffs against its
    /// own back buffer, but the shadow also lets us survive resize-driven
    /// full redraws cheaply.
    shadow: Vec<Cell>,
    /// Last `font` string we set on the context. `ctx.set_font` is one of
    /// the more expensive context properties — gating updates on this lets
    /// us skip the no-op writes for cells whose modifiers don't change the
    /// font.
    current_font: String,
    /// Scratch buffer for building the font string, reused across cells to
    /// keep wasm heap allocations bounded.
    font_buf: String,
    /// Scratch buffer for building CSS colour strings.
    color_buf: String,
    /// Cell rects we stamped emoji glyphs into during the previous overlay
    /// pass, outside the selection list's emoji column band (which is
    /// cleared wholesale via [`App::emoji_overlay_band`]). Used to wipe
    /// stale toast glyphs once the toast expires; in overlay mode the
    /// buffer never reports those positions as changed, so the diff loop
    /// can't clean them up for us.
    prev_emoji_rects: Vec<Rect>,
}

impl CanvasBackend {
    /// Build a backend that paints into the `<canvas>` element with id
    /// `canvas_id`. The canvas's CSS-defined size determines the grid
    /// dimensions; call [`Self::refresh_geometry`] after CSS-affecting
    /// changes (e.g. `window.resize`) to pick the new size up.
    pub fn new(canvas_id: &str) -> Result<Self, String> {
        let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
        let document = window.document().ok_or_else(|| "no document".to_string())?;
        let canvas = lookup_canvas(&document, canvas_id)?;
        let ctx = canvas
            .get_context("2d")
            .map_err(jsv_to_string)?
            .ok_or_else(|| "canvas has no 2d context".to_string())?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "canvas context is not 2d".to_string())?;

        let mut backend = Self {
            canvas,
            ctx,
            size: Size::new(0, 0),
            cell_w: 0.0,
            cell_h: 0.0,
            shadow: Vec::new(),
            current_font: String::new(),
            font_buf: String::with_capacity(160),
            color_buf: String::with_capacity(32),
            prev_emoji_rects: Vec::new(),
        };
        backend.refresh_geometry()?;
        Ok(backend)
    }

    /// The underlying `<canvas>` element. Mouse code uses this to translate
    /// viewport coordinates into cell coordinates via the cached geometry.
    pub fn canvas_element(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    /// Paint emoji glyphs at their cell-coordinate rectangles, on top of
    /// whatever the diff loop has already drawn, after first clearing the
    /// selection list's emoji column `band` and any rects the previous
    /// pass painted outside the band (e.g. toast glyphs).
    ///
    /// Used by the overlay flow in [`crate::run`]: ratatui's column
    /// accounting depends on `unicode-width`, which under-counts the
    /// rendered width of VS16 / ZWJ emoji sequences. The picker leaves
    /// the emoji column blank in the buffer (`EmojiSource::Overlay`) so
    /// column alignment stays exact regardless of the glyph's actual
    /// rendered width.
    ///
    /// The blank-cell choice has a consequence: ratatui's diff never
    /// reports those cells as changed, so when the visible-emoji list
    /// shrinks (e.g. a filter narrows the results) the backend never
    /// hears about the now-empty positions. This function pays that cost
    /// by wiping the whole band each frame before re-painting visible
    /// glyphs; in this picker the highlighted-row style only changes
    /// `fg`, not `bg`, so cleared positions are correct as-is
    /// (transparent → page background shows through).
    ///
    /// `fillTextWithMaxWidth` is used instead of plain `fillText` so
    /// glyphs whose natural pixel width exceeds the reserved column get
    /// scaled down to fit instead of bleeding into the next column.
    pub fn paint_emoji_overlays<'a, I>(&mut self, band: Option<Rect>, overlays: I)
    where
        I: IntoIterator<Item = (Rect, &'a str)>,
    {
        if self.cell_w <= 0.0 {
            return;
        }
        // Clear the full emoji column band the picker reserves. This wipes
        // any stale glyphs from rows that fell out of the visible list — in
        // overlay mode those buffer cells stay blank, so ratatui's diff
        // never asks us to repaint them.
        if let Some(band) = band {
            self.ctx.clear_rect(
                band.x as f64 * self.cell_w,
                band.y as f64 * self.cell_h,
                band.width as f64 * self.cell_w,
                band.height as f64 * self.cell_h,
            );
        }
        // Also clear individual rects from the previous pass — covers any
        // overlay emojis painted outside the band, e.g. the toast glyph.
        for rect in self.prev_emoji_rects.drain(..) {
            self.ctx.clear_rect(
                rect.x as f64 * self.cell_w,
                rect.y as f64 * self.cell_h,
                rect.width as f64 * self.cell_w,
                rect.height as f64 * self.cell_h,
            );
        }
        self.apply_font(false, false);
        // `fillStyle` must be set for `fillText`, but colour-emoji glyphs
        // paint their own pixels and ignore it; pick anything visible.
        self.ctx.set_fill_style_str("rgb(255,255,255)");
        for (cell, emoji) in overlays {
            if emoji.is_empty() {
                continue;
            }
            let px = cell.x as f64 * self.cell_w;
            let py = cell.y as f64 * self.cell_h;
            let max_w = cell.width as f64 * self.cell_w;
            let _ = self.ctx.fill_text_with_max_width(emoji, px, py, max_w);
            self.prev_emoji_rects.push(cell);
        }
    }

    /// Forget the overlay rects recorded by the previous pass without
    /// wiping them.
    ///
    /// Callers use this together with [`Backend::clear`]: once the whole
    /// canvas is cleared and repainted from scratch, there is nothing stale
    /// left to wipe, and applying the old rects *after* that repaint would
    /// punch transparent holes into fresh pixels — pixels neither ratatui's
    /// buffer nor [`Self::shadow`] would know to repaint.
    pub fn clear_overlay_rects(&mut self) {
        self.prev_emoji_rects.clear();
    }

    /// Current grid geometry. Mouse code reads this to convert from pixel
    /// coordinates to cell coordinates without re-measuring.
    pub fn geometry(&self) -> Geometry {
        Geometry {
            cell_w: self.cell_w,
            cell_h: self.cell_h,
            cols: self.size.width,
            rows: self.size.height,
        }
    }

    /// Re-read the canvas's CSS bounding rect and device-pixel ratio,
    /// resize the drawing buffer, re-measure a cell, and resize the shadow
    /// buffer. Called from `new` and from the window `resize` listener.
    pub fn refresh_geometry(&mut self) -> Result<(), String> {
        let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
        let dpr = window.device_pixel_ratio().max(1.0);

        let rect = self.canvas.get_bounding_client_rect();
        let css_w = rect.width().max(1.0);
        let css_h = rect.height().max(1.0);

        // The drawing buffer is sized in device pixels; the `setTransform`
        // below makes our (CSS-pixel) coordinates land on the right device
        // pixels. Resetting `width`/`height` clears the buffer and resets
        // all context state, which is why we re-set font/baseline/transform
        // after.
        self.canvas.set_width((css_w * dpr).round() as u32);
        self.canvas.set_height((css_h * dpr).round() as u32);
        self.ctx
            .set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0)
            .map_err(jsv_to_string)?;
        self.ctx.set_text_baseline("top");
        // Force the next paint to re-set the font; the canvas reset above
        // dropped it.
        self.current_font.clear();
        self.apply_font(false, false);

        let metrics = self.ctx.measure_text("0").map_err(jsv_to_string)?;
        let cell_w = metrics.width().max(1.0);
        let cell_h = (FONT_SIZE_PX * LINE_HEIGHT_FACTOR).round();

        let cols = ((css_w / cell_w).floor() as i64).clamp(1, u16::MAX as i64) as u16;
        let rows = ((css_h / cell_h).floor() as i64).clamp(1, u16::MAX as i64) as u16;

        self.cell_w = cell_w;
        self.cell_h = cell_h;
        self.size = Size::new(cols, rows);
        self.shadow = vec![Cell::default(); cols as usize * rows as usize];
        Ok(())
    }

    fn apply_font(&mut self, bold: bool, italic: bool) {
        use std::fmt::Write;

        self.font_buf.clear();
        if italic {
            self.font_buf.push_str("italic ");
        }
        if bold {
            self.font_buf.push_str("bold ");
        }
        let _ = write!(self.font_buf, "{}px {}", FONT_SIZE_PX as u32, FONT_FAMILIES);
        if self.current_font != self.font_buf {
            self.ctx.set_font(&self.font_buf);
            self.current_font.clear();
            self.current_font.push_str(&self.font_buf);
        }
    }

    fn paint_cell(&mut self, x: u16, y: u16, cell: &Cell) {
        let symbol = cell.symbol();
        // unicode-width returns 0 for the trailing column of a double-width
        // cell, but Buffer::diff doesn't hand us those (see module docs).
        // Treat 0 as 1 anyway so an unexpected empty symbol doesn't fail to
        // clear its background.
        let width_cells = UnicodeWidthStr::width(symbol).max(1) as f64;
        let px = x as f64 * self.cell_w;
        let py = y as f64 * self.cell_h;
        let rect_w = self.cell_w * width_cells;
        let rect_h = self.cell_h;

        let (mut fg, mut bg) = (cell.fg, cell.bg);
        if cell.modifier.contains(Modifier::REVERSED) {
            std::mem::swap(&mut fg, &mut bg);
        }

        // Background. For `Color::Reset` we clear instead of filling so the
        // page background (set by CSS on the canvas's parent) shows
        // through.
        if bg == Color::Reset {
            self.ctx.clear_rect(px, py, rect_w, rect_h);
        } else {
            write_color_css(&mut self.color_buf, bg);
            self.ctx.set_fill_style_str(&self.color_buf);
            self.ctx.fill_rect(px, py, rect_w, rect_h);
        }

        if cell.modifier.contains(Modifier::HIDDEN) || symbol.is_empty() || symbol == " " {
            return;
        }

        self.apply_font(
            cell.modifier.contains(Modifier::BOLD),
            cell.modifier.contains(Modifier::ITALIC),
        );
        // Reset fg defaults to white so light glyphs remain visible against
        // dark backgrounds without the picker explicitly setting a colour.
        let fg_color = if fg == Color::Reset { Color::White } else { fg };
        write_color_css(&mut self.color_buf, fg_color);
        self.ctx.set_fill_style_str(&self.color_buf);

        let dim = cell.modifier.contains(Modifier::DIM);
        if dim {
            self.ctx.set_global_alpha(0.5);
        }
        let _ = self.ctx.fill_text(symbol, px, py);
        if dim {
            self.ctx.set_global_alpha(1.0);
        }

        if cell.modifier.contains(Modifier::UNDERLINED) {
            self.ctx.fill_rect(px, py + rect_h - 1.0, rect_w, 1.0);
        }
        if cell.modifier.contains(Modifier::CROSSED_OUT) {
            self.ctx
                .fill_rect(px, py + (rect_h * 0.5).round(), rect_w, 1.0);
        }
    }
}

impl Backend for CanvasBackend {
    type Error = IoError;

    fn draw<'a, I>(&mut self, content: I) -> IoResult<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let width = self.size.width as usize;
        let total = self.shadow.len();
        for (x, y, cell) in content {
            let idx = (y as usize) * width + (x as usize);
            if idx >= total {
                // Out-of-range coordinates can briefly appear during a
                // resize race; the next frame will paint the new geometry.
                continue;
            }
            if cells_equal(&self.shadow[idx], cell) {
                continue;
            }
            self.paint_cell(x, y, cell);
            self.shadow[idx].clone_from(cell);
        }
        Ok(())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn hide_cursor(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn show_cursor(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn get_cursor_position(&mut self) -> IoResult<Position> {
        Ok(Position::new(0, 0))
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, _position: P) -> IoResult<()> {
        Ok(())
    }

    fn clear(&mut self) -> IoResult<()> {
        self.shadow.iter_mut().for_each(|c| *c = Cell::default());
        self.ctx.clear_rect(
            0.0,
            0.0,
            self.size.width as f64 * self.cell_w,
            self.size.height as f64 * self.cell_h,
        );
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> IoResult<()> {
        match clear_type {
            ClearType::All => self.clear(),
            _ => Err(IoError::other("unimplemented")),
        }
    }

    fn size(&self) -> IoResult<Size> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> IoResult<WindowSize> {
        unimplemented!("window_size is not used by the picker")
    }
}

fn cells_equal(a: &Cell, b: &Cell) -> bool {
    a.symbol() == b.symbol() && a.fg == b.fg && a.bg == b.bg && a.modifier == b.modifier
}

fn lookup_canvas(document: &Document, id: &str) -> Result<HtmlCanvasElement, String> {
    document
        .get_element_by_id(id)
        .ok_or_else(|| format!("element #{id} not found"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| format!("element #{id} is not a <canvas>"))
}

/// Write a CSS `rgb(...)` colour string for `color` into `buf`, replacing
/// any prior contents. Reusing the buffer keeps per-cell allocations off
/// the wasm heap during heavy redraws.
fn write_color_css(buf: &mut String, color: Color) {
    use std::fmt::Write;

    buf.clear();
    let (r, g, b) = color_to_rgb(color).unwrap_or((255, 255, 255));
    let _ = write!(buf, "rgb({r},{g},{b})");
}

/// Convert a ratatui [`Color`] into 24-bit RGB. Returns `None` for
/// [`Color::Reset`] so the caller can pick a default appropriate for the
/// context (foreground vs background) instead of materialising one here.
fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Reset => None,
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Black => Some((0, 0, 0)),
        Color::Red => Some((128, 0, 0)),
        Color::Green => Some((0, 128, 0)),
        Color::Yellow => Some((128, 128, 0)),
        Color::Blue => Some((0, 0, 128)),
        Color::Magenta => Some((128, 0, 128)),
        Color::Cyan => Some((0, 128, 128)),
        Color::Gray => Some((192, 192, 192)),
        Color::DarkGray => Some((128, 128, 128)),
        Color::LightRed => Some((255, 0, 0)),
        Color::LightGreen => Some((0, 255, 0)),
        Color::LightYellow => Some((255, 255, 0)),
        Color::LightBlue => Some((0, 0, 255)),
        Color::LightMagenta => Some((255, 0, 255)),
        Color::LightCyan => Some((0, 255, 255)),
        Color::White => Some((255, 255, 255)),
        Color::Indexed(code) => Some(indexed_color_to_rgb(code)),
    }
}

/// Map the 256-colour palette to RGB. Mirrors xterm's `256colres.h`.
fn indexed_color_to_rgb(index: u8) -> (u8, u8, u8) {
    const BASIC: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    match index {
        0..=15 => BASIC[index as usize],
        16..=231 => {
            let i = index - 16;
            let r = i / 36;
            let g = (i % 36) / 6;
            let b = i % 6;
            let component = |n: u8| if n == 0 { 0u8 } else { 55 + 40 * n };
            (component(r), component(g), component(b))
        }
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            (gray, gray, gray)
        }
    }
}

fn jsv_to_string(v: JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{v:?}"))
}
