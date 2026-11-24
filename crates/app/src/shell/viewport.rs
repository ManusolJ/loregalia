use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gpui_kit::{Pixels, ScrollHandle, point, px};

const OVERSCAN: usize = 3;
const MOVED: f32 = 0.5;

#[derive(Clone, Default)]
pub(super) struct Viewport {
    pub(super) scroll: ScrollHandle,
    pub(super) height: f32,
    top: Rc<Cell<f32>>,
}

impl Viewport {
    pub(super) fn rows(&self, pitch: f32, total: usize) -> Range<usize> {
        let scrolled = -f32::from(self.scroll.offset().y);

        visible_rows(scrolled - self.top.get(), self.height, pitch, total)
    }

    pub(super) fn to_top(&self) {
        self.scroll.set_offset(point(px(0.0), px(0.0)));
    }

    pub(super) fn place_list(&self, list_top: Pixels) -> bool {
        let page = self.scroll.bounds().origin.y + self.scroll.offset().y;
        let top = f32::from(list_top - page);
        let moved = (top - self.top.get()).abs() > MOVED;
        self.top.set(top);
        moved
    }

    pub(super) fn reflow(&self, before: usize, after: usize, pitch: f32) {
        let offset = self.scroll.offset();
        let into_list = -f32::from(offset.y) - self.top.get();
        if into_list <= 0.0 {
            return;
        }

        let wanted = self.top.get() + reflowed(into_list, before, after, pitch);
        self.scroll.set_offset(point(offset.x, px(-wanted)));
    }

    pub(super) fn reveal(&self, row: usize, pitch: f32) {
        let offset = self.scroll.offset();
        let scrolled = -f32::from(offset.y);
        let page = f32::from(self.scroll.bounds().size.height);
        let start = self.top.get() + pitch * row as f32;

        if let Some(wanted) = revealed(scrolled, page, start, pitch) {
            self.scroll.set_offset(point(offset.x, px(-wanted)));
        }
    }
}

fn visible_rows(scrolled: f32, height: f32, pitch: f32, total: usize) -> Range<usize> {
    let shown = (height / pitch).ceil() as usize + 1;
    let first = ((scrolled / pitch).floor().max(0.0) as usize).min(total.saturating_sub(shown));

    first.saturating_sub(OVERSCAN)..(first + shown + OVERSCAN).min(total)
}

fn reflowed(into_list: f32, before: usize, after: usize, pitch: f32) -> f32 {
    let row = (into_list / pitch).floor();
    let item = row as usize * before;

    (item / after.max(1)) as f32 * pitch + (into_list - row * pitch)
}

fn revealed(scrolled: f32, page: f32, start: f32, size: f32) -> Option<f32> {
    if page <= 0.0 {
        None
    } else if start < scrolled {
        Some(start.max(0.0))
    } else if start + size > scrolled + page {
        Some(start + size - page)
    } else {
        None
    }
}

#[derive(Debug, Default)]
pub(super) struct Reloads {
    in_flight: usize,
    overlapped: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Arrival {
    Apply,
    Wait,
    LoadAgain,
}

impl Reloads {
    pub(super) fn begin(&mut self) {
        self.in_flight += 1;
    }

    pub(super) fn interrupt(&mut self) {
        if self.in_flight > 0 {
            self.overlapped = true;
        }
    }

    pub(super) fn finish(&mut self) -> Arrival {
        self.in_flight = self.in_flight.saturating_sub(1);
        if self.in_flight > 0 {
            self.overlapped = true;
            Arrival::Wait
        } else if std::mem::take(&mut self.overlapped) {
            Arrival::LoadAgain
        } else {
            Arrival::Apply
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Arrival, Reloads, reflowed, revealed, visible_rows};

    #[test]
    fn a_change_of_columns_keeps_the_same_items_at_the_top() {
        assert_eq!(reflowed(1_000.0, 7, 4, 100.0), 1_700.0);
        assert_eq!(reflowed(1_750.0, 4, 7, 100.0), 950.0);
        assert_eq!(reflowed(250.0, 3, 3, 100.0), 250.0);
    }

    #[test]
    fn only_rows_near_the_window_are_drawn() {
        assert_eq!(visible_rows(0.0, 800.0, 100.0, 1_000), 0..12);
        assert_eq!(visible_rows(5_000.0, 800.0, 100.0, 1_000), 47..62);
    }

    #[test]
    fn the_range_stays_inside_the_list() {
        assert_eq!(visible_rows(0.0, 800.0, 100.0, 0), 0..0);
        assert_eq!(visible_rows(0.0, 800.0, 100.0, 4), 0..4);
        assert_eq!(visible_rows(-300.0, 800.0, 100.0, 1_000), 0..12);
        assert_eq!(visible_rows(9_999_999.0, 800.0, 100.0, 1_000), 988..1_000);
    }

    #[test]
    fn rows_below_a_shelf_start_at_the_first_one() {
        assert_eq!(visible_rows(-320.0, 800.0, 100.0, 50).start, 0);
    }

    #[test]
    fn revealing_moves_only_as_far_as_needed() {
        assert_eq!(revealed(500.0, 800.0, 600.0, 100.0), None);
        assert_eq!(revealed(500.0, 800.0, 300.0, 100.0), Some(300.0));
        assert_eq!(revealed(500.0, 800.0, 1_250.0, 100.0), Some(550.0));
        assert_eq!(revealed(500.0, 0.0, 1_250.0, 100.0), None);
    }

    #[test]
    fn a_single_reload_is_applied() {
        let mut reloads = Reloads::default();
        reloads.begin();

        assert_eq!(reloads.finish(), Arrival::Apply);
    }

    #[test]
    fn overlapping_reloads_load_once_more_after_the_last() {
        let mut reloads = Reloads::default();
        reloads.begin();
        reloads.begin();
        reloads.begin();

        assert_eq!(reloads.finish(), Arrival::Wait);
        assert_eq!(reloads.finish(), Arrival::Wait);
        assert_eq!(reloads.finish(), Arrival::LoadAgain);
        reloads.begin();
        assert_eq!(reloads.finish(), Arrival::Apply);
    }

    #[test]
    fn a_change_made_during_a_reload_loads_again() {
        let mut reloads = Reloads::default();
        reloads.interrupt();
        reloads.begin();
        reloads.interrupt();

        assert_eq!(reloads.finish(), Arrival::LoadAgain);
        reloads.begin();
        assert_eq!(reloads.finish(), Arrival::Apply);
    }
}
