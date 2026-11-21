// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Mouse-driven pane management: divider hit-testing, percentage
//! conversion and clamping for drag-resizing, shared between input
//! handling and rendering.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Which pane divider a mouse interaction targets.
///
/// The abutting border cells of adjacent panes are the drag handles;
/// there is no gutter, so no space is lost to divider widgets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Divider {
    #[default]
    None,
    /// Between the service types panel and the right column.
    Vertical,
    /// Between the services list and the details pane.
    Horizontal,
}

/// Last-frame pane geometry for mouse hit-testing, recorded during the
/// per-frame layout preparation so the mouse acts on the frame as drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaneRects {
    /// The area split into the types panel and the right column.
    pub body: Rect,
    pub types: Rect,
    pub services: Rect,
    pub details: Rect,
}

impl PaneRects {
    /// The right column: the services list and the details pane share
    /// their origin and width, so its rect is their union.
    pub fn right_column(&self) -> Rect {
        Rect::new(
            self.services.x,
            self.services.y,
            self.services.width,
            self.services.height.saturating_add(self.details.height),
        )
    }
}

/// Percentage of `body`'s width at column `col` (0-100).
pub fn pct_from_x(col: u16, body: Rect) -> u16 {
    if body.width == 0 {
        // Degenerate frame: nothing is draggable or grabbable, so the
        // fallback value is arbitrary.
        return 33;
    }
    (col.saturating_sub(body.x) as u32 * 100 / body.width.max(1) as u32).min(100) as u16
}

/// Percentage of `column`'s height at row `row` (0-100).
pub fn pct_from_y(row: u16, column: Rect) -> u16 {
    if column.height == 0 {
        // Degenerate frame: nothing is draggable or grabbable, so the
        // fallback value is arbitrary.
        return 34;
    }
    (row.saturating_sub(column.y) as u32 * 100 / column.height.max(1) as u32).min(100) as u16
}

/// Percentage band kept for the types panel; the right column always
/// keeps the remainder, so narrow terminals cannot collapse either side.
pub const TYPES_WIDTH_MIN_PCT: u16 = 15;
pub const TYPES_WIDTH_MAX_PCT: u16 = 80;

pub fn clamp_types_width_pct(pct: u16) -> u16 {
    pct.clamp(TYPES_WIDTH_MIN_PCT, TYPES_WIDTH_MAX_PCT)
}

/// Static percentage band for the services share, used when the pane
/// geometry is unusable; with real geometry the row floors below win.
pub const SERVICES_HEIGHT_MIN_PCT: u16 = 10;
pub const SERVICES_HEIGHT_MAX_PCT: u16 = 90;

/// Drag-time percentage band for the services share: the services list
/// keeps at least `min_total_rows` (content + borders) and the details
/// pane keeps at least `details_min_rows`. On tiny columns the band
/// inverts and the services floor wins, mirroring the default layout.
fn services_pct_band(column_height: u16, min_total_rows: u16, details_min_rows: u16) -> (u16, u16) {
    if column_height == 0 {
        return (SERVICES_HEIGHT_MIN_PCT, SERVICES_HEIGHT_MAX_PCT);
    }
    let h = column_height as u32;
    let lo = (min_total_rows as u32 * 100).div_ceil(h) as u16;
    let lo = lo.clamp(SERVICES_HEIGHT_MIN_PCT, 100);
    let hi = (column_height.saturating_sub(details_min_rows) as u32 * 100 / h) as u16;
    let hi = hi.clamp(SERVICES_HEIGHT_MIN_PCT, SERVICES_HEIGHT_MAX_PCT);
    (lo, hi.max(lo))
}

/// Clamp a stored services share against the row floors.
pub fn clamp_services_height_pct(
    pct: u16,
    column_height: u16,
    min_total_rows: u16,
    details_min_rows: u16,
) -> u16 {
    let (lo, hi) = services_pct_band(column_height, min_total_rows, details_min_rows);
    pct.clamp(lo, hi)
}

/// Total services-list rows (content + borders) for a stored share of
/// the right column's height, re-clamped against the absolute floors so
/// a terminal resize can never collapse either pane.
pub fn services_height_from_pct(
    pct: u16,
    column_height: u16,
    min_total_rows: u16,
    details_min_rows: u16,
) -> u16 {
    if column_height == 0 {
        return min_total_rows;
    }
    let effective = clamp_services_height_pct(pct, column_height, min_total_rows, details_min_rows);
    let rows = (column_height as u32 * effective as u32 / 100) as u16;
    let max_rows = column_height
        .saturating_sub(details_min_rows)
        .max(min_total_rows);
    rows.clamp(min_total_rows, max_rows)
}

/// Width of the types panel for a stored share of the body width,
/// clamped to the same absolute band the content-fit default honors: a
/// minimum panel width, and a details pane floor of `details_min_width`
/// columns.
pub fn types_width_from_pct(
    pct: u16,
    body_width: u16,
    min_width: u16,
    details_min_width: u16,
) -> u16 {
    let raw = (body_width as u32 * clamp_types_width_pct(pct) as u32 / 100) as u16;
    let max = body_width.saturating_sub(details_min_width).max(min_width);
    raw.clamp(min_width, max)
}

/// Shared pane border under the cursor, if any. With no gutter the two
/// abutting border cells form a 2-cell grab zone: the types panel's
/// right edge / the right column's left edge vertically, and the
/// services list's bottom / the details top horizontally. Seams win
/// over panes so the border stays grabbable.
pub fn divider_at(panes: &PaneRects, col: u16, row: u16) -> Divider {
    let types = panes.types;
    let services = panes.services;
    let details = panes.details;
    if types.width > 0 && services.width > 0 {
        let types_edge = types.x.saturating_add(types.width.saturating_sub(1));
        if (col == types_edge || col == services.x)
            && row >= types.y
            && row < types.y.saturating_add(types.height)
        {
            return Divider::Vertical;
        }
    }
    if services.height > 0 && details.height > 0 {
        let services_bottom = services.y.saturating_add(services.height.saturating_sub(1));
        if (row == services_bottom || row == details.y)
            && col >= services.x
            && col < services.x.saturating_add(services.width)
        {
            return Divider::Horizontal;
        }
    }
    Divider::None
}

/// Whether a point falls inside a rect.
pub fn point_in(rect: Rect, col: u16, row: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

/// Border style for a pane adjacent to a divider: the base style unless
/// that divider is hovered or dragged, when a subtle cyan seam takes
/// over; without colors reverse video carries the affordance.
pub fn resize_border_style(base: Style, active: bool, no_color: bool) -> Style {
    if !active {
        base
    } else if no_color {
        base.add_modifier(Modifier::REVERSED)
    } else {
        Style::default().fg(Color::Cyan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panes() -> PaneRects {
        // 100x40 body: an 11-wide types panel, then the right column
        // split into a 7-row services list and the details pane.
        PaneRects {
            body: Rect::new(0, 0, 100, 40),
            types: Rect::new(0, 0, 11, 40),
            services: Rect::new(11, 0, 89, 7),
            details: Rect::new(11, 7, 89, 33),
        }
    }

    #[test]
    fn test_pct_from_x_maps_column_to_percentage() {
        let body = Rect::new(0, 3, 100, 20);
        assert_eq!(pct_from_x(5, body), 5);
        assert_eq!(pct_from_x(50, body), 50);
        assert_eq!(pct_from_x(100, body), 100);
    }

    #[test]
    fn test_pct_from_y_maps_row_to_percentage() {
        let column = Rect::new(33, 3, 67, 30);
        assert_eq!(pct_from_y(3, column), 0);
        assert_eq!(pct_from_y(3 + 15, column), 50);
        assert_eq!(pct_from_y(3 + 30, column), 100);
    }

    #[test]
    fn test_pct_from_zero_area_falls_back() {
        assert_eq!(pct_from_x(7, Rect::new(0, 0, 0, 10)), 33);
        assert_eq!(pct_from_y(7, Rect::new(0, 0, 10, 0)), 34);
    }

    #[test]
    fn test_divider_at_detects_vertical_seam() {
        let panes = panes();
        // The two abutting border cells form the grab zone.
        assert_eq!(divider_at(&panes, 10, 20), Divider::Vertical);
        assert_eq!(divider_at(&panes, 11, 20), Divider::Vertical);
    }

    #[test]
    fn test_divider_at_detects_horizontal_seam() {
        let panes = panes();
        assert_eq!(divider_at(&panes, 50, 6), Divider::Horizontal);
        assert_eq!(divider_at(&panes, 50, 7), Divider::Horizontal);
    }

    #[test]
    fn test_divider_at_misses_pane_interiors_and_edges() {
        let rects = panes();
        assert_eq!(divider_at(&rects, 5, 20), Divider::None);
        assert_eq!(divider_at(&rects, 50, 3), Divider::None);
        assert_eq!(divider_at(&rects, 50, 30), Divider::None);
        // Crossing point of both seams: the vertical seam wins.
        assert_eq!(divider_at(&rects, 11, 6), Divider::Vertical);
        // Outside the panes' shared span there is no seam.
        let mut collapsed = panes();
        collapsed.types = Rect::new(0, 0, 0, 0);
        assert_eq!(divider_at(&collapsed, 10, 20), Divider::None);
    }

    #[test]
    fn test_point_in_rect() {
        let rect = Rect::new(3, 4, 10, 5);
        assert!(point_in(rect, 3, 4));
        assert!(point_in(rect, 12, 8));
        assert!(!point_in(rect, 13, 4));
        assert!(!point_in(rect, 3, 9));
        assert!(!point_in(rect, 2, 4));
        assert!(!point_in(Rect::default(), 0, 0));
    }

    #[test]
    fn test_right_column_unions_services_and_details() {
        let panes = panes();
        assert_eq!(
            panes.right_column(),
            Rect::new(11, 0, 89, 40),
            "the right column spans the services list and the details pane"
        );
    }

    #[test]
    fn test_clamp_types_width_pct_bounds() {
        assert_eq!(clamp_types_width_pct(5), 15);
        assert_eq!(clamp_types_width_pct(40), 40);
        assert_eq!(clamp_types_width_pct(95), 80);
    }

    #[test]
    fn test_clamp_services_height_pct_derives_floors() {
        // 40-row column: services keeps 7 rows (5 content + borders),
        // details keeps 8: band [18, 80].
        assert_eq!(clamp_services_height_pct(5, 40, 7, 8), 18);
        assert_eq!(clamp_services_height_pct(50, 40, 7, 8), 50);
        assert_eq!(clamp_services_height_pct(97, 40, 7, 8), 80);
        // Degenerate geometry: the static percentage band applies.
        assert_eq!(clamp_services_height_pct(97, 0, 7, 8), 90);
    }

    #[test]
    fn test_types_width_from_pct_applies_absolute_floors() {
        // The content-fit default caps the panel so details keep 30
        // columns: at width 100 the drag band tightens to [10, 70].
        assert_eq!(types_width_from_pct(15, 100, 10, 30), 15);
        assert_eq!(types_width_from_pct(80, 100, 10, 30), 70);
        assert_eq!(types_width_from_pct(40, 200, 10, 30), 80);
        // Tiny terminal: the panel keeps at least its minimum width.
        assert_eq!(types_width_from_pct(50, 20, 10, 30), 10);
    }

    #[test]
    fn test_services_height_from_pct_applies_absolute_floors() {
        // 50% of a 40-row column is 20 rows; the floors never bind.
        assert_eq!(services_height_from_pct(50, 40, 7, 8), 20);
        // Below the services minimum the floor wins.
        assert_eq!(services_height_from_pct(5, 40, 7, 8), 7);
        // Above the details minimum the ceiling wins.
        assert_eq!(services_height_from_pct(97, 40, 7, 8), 32);
        // Stored shares keep their floors when the terminal shrinks.
        assert_eq!(services_height_from_pct(80, 12, 7, 8), 7);
    }

    #[test]
    fn test_resize_border_style_variants() {
        use ratatui::style::{Color, Modifier, Style};

        let base = Style::default().fg(Color::Gray);
        // Inactive: the base style carries the border.
        assert_eq!(resize_border_style(base, false, false), base);
        assert_eq!(resize_border_style(base, false, true), base);
        // Active with colors: the subtle cyan seam.
        assert_eq!(resize_border_style(base, true, false).fg, Some(Color::Cyan));
        // Active without colors: reverse video carries the affordance.
        assert!(
            resize_border_style(base, true, true)
                .add_modifier
                .contains(Modifier::REVERSED)
        );
    }
}
