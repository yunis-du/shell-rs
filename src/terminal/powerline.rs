//! Solid Powerline separators join the adjacent cell's background. Draw
//! them at the grid's size so font metrics and extra line spacing cannot
//! leave a step where the two colors meet.

use gpui_kit::{Bounds, Path, PathBuilder, Pixels, point, px};

#[derive(Clone, Copy)]
pub(super) enum Separator {
    RightArrow,
    LeftArrow,
    RightCap,
    LeftCap,
}

impl Separator {
    pub(super) fn from_char(character: char) -> Option<Self> {
        match character {
            '\u{e0b0}' => Some(Self::RightArrow),
            '\u{e0b2}' => Some(Self::LeftArrow),
            '\u{e0b4}' => Some(Self::RightCap),
            '\u{e0b6}' => Some(Self::LeftCap),
            _ => None,
        }
    }

    pub(super) fn path(self, cell: Bounds<Pixels>) -> Option<Path<Pixels>> {
        let right = matches!(self, Self::RightArrow | Self::RightCap);
        let (join, tip) = if right {
            (cell.left(), cell.right())
        } else {
            (cell.right(), cell.left())
        };
        let half_height = cell.size.height / 2.;
        let tip = point(tip, cell.top() + half_height);
        let bottom = point(join, cell.bottom());
        let mut path = PathBuilder::fill();
        path.move_to(point(join, cell.top()));
        match self {
            Self::RightArrow | Self::LeftArrow => {
                path.line_to(tip);
                path.line_to(bottom);
            }
            Self::RightCap | Self::LeftCap => {
                // The half ellipse has a flat side the full height of the
                // adjacent background and a tip at the center of this cell.
                let radii = point(cell.size.width, half_height);
                path.arc_to(radii, px(0.), false, right, tip);
                path.arc_to(radii, px(0.), false, right, bottom);
            }
        }
        path.close();
        path.build().ok()
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Bounds, Pixels, Point, point, px, size};

    use super::Separator;

    #[test]
    fn solid_separators_join_the_full_row_at_either_line_height() {
        for height in [13., 23.5] {
            let cell = Bounds::new(point(px(24.5), px(11.)), size(px(8.), px(height)));
            for (character, right) in [
                ('\u{e0b0}', true),
                ('\u{e0b2}', false),
                ('\u{e0b4}', true),
                ('\u{e0b6}', false),
            ] {
                let path = Separator::from_char(character).unwrap().path(cell).unwrap();
                let (join, tip) = if right {
                    (cell.left(), cell.right())
                } else {
                    (cell.right(), cell.left())
                };
                // The tessellator can round the ellipse's endpoints slightly.
                let close = |a: Point<Pixels>, b: Point<Pixels>| {
                    (a.x - b.x).as_f32().abs() < 0.001 && (a.y - b.y).as_f32().abs() < 0.001
                };
                let vertex_at = |x, y| {
                    path.vertices
                        .iter()
                        .any(|vertex| close(vertex.xy_position, point(x, y)))
                };
                assert!(vertex_at(join, cell.top()));
                assert!(vertex_at(join, cell.bottom()));
                assert!(vertex_at(tip, cell.center().y), "{character:?}: {path:?}");
                assert!(close(path.bounds.origin, cell.origin));
                assert!(close(path.bounds.bottom_right(), cell.bottom_right()));
            }
        }
    }
}
