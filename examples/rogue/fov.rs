//! Field of view by recursive shadowcasting on a square grid.
//! Computes the set of cells visible from a given origin within a
//! fixed radius. The implementation walks eight octants and casts
//! shadows from any opaque cell encountered. Symmetric in the
//! sense that if A sees B then B sees A.
//!
//! References. The algorithm follows the standard description on
//! the RogueBasin wiki under "FOV using recursive shadowcasting"
//! and the matching implementations in Brogue and AngBand.

use crate::world::Map;
use crate::{MAP_H, MAP_W};

const OCTANT_MULTIPLIERS: [[i32; 4]; 8] = [
    [1, 0, 0, 1],
    [0, 1, 1, 0],
    [0, -1, 1, 0],
    [-1, 0, 0, 1],
    [-1, 0, 0, -1],
    [0, -1, -1, 0],
    [0, 1, -1, 0],
    [1, 0, 0, -1],
];

/// Compute visibility flags from origin `(ox, oy)` within `radius`
/// tiles. The supplied `visible` buffer must have length
/// `MAP_W * MAP_H`. The buffer is reset to `false` before casting.
pub fn compute(map: &Map, ox: i32, oy: i32, radius: i32, visible: &mut [bool]) {
    let total = (MAP_W * MAP_H) as usize;
    debug_assert_eq!(visible.len(), total);
    for slot in visible.iter_mut() {
        *slot = false;
    }
    set_visible(visible, ox, oy);
    for mult in OCTANT_MULTIPLIERS.iter() {
        cast_light(
            map, visible, ox, oy, radius, 1, 1.0, 0.0, mult[0], mult[1], mult[2], mult[3],
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn cast_light(
    map: &Map,
    visible: &mut [bool],
    ox: i32,
    oy: i32,
    radius: i32,
    row_start: i32,
    mut start_slope: f64,
    end_slope: f64,
    xx: i32,
    xy: i32,
    yx: i32,
    yy: i32,
) {
    if start_slope < end_slope {
        return;
    }
    let radius_sq = (radius * radius) as f64;
    let mut new_start_slope = start_slope;
    for row in row_start..=radius {
        let mut blocked = false;
        for dx in -row..=0 {
            let dy = -row;
            let cell_x = ox + dx * xx + dy * xy;
            let cell_y = oy + dx * yx + dy * yy;
            if cell_x < 0 || cell_y < 0 || cell_x >= MAP_W as i32 || cell_y >= MAP_H as i32 {
                continue;
            }
            let l_slope = (dx as f64 - 0.5) / (dy as f64 + 0.5);
            let r_slope = (dx as f64 + 0.5) / (dy as f64 - 0.5);
            if start_slope < r_slope {
                continue;
            }
            if end_slope > l_slope {
                break;
            }
            let dist_sq = (dx * dx + dy * dy) as f64;
            if dist_sq < radius_sq {
                set_visible(visible, cell_x, cell_y);
            }
            if blocked {
                if !map.is_transparent(cell_x, cell_y) {
                    new_start_slope = r_slope;
                    continue;
                } else {
                    blocked = false;
                    start_slope = new_start_slope;
                }
            } else if !map.is_transparent(cell_x, cell_y) && row < radius {
                blocked = true;
                cast_light(
                    map,
                    visible,
                    ox,
                    oy,
                    radius,
                    row + 1,
                    start_slope,
                    l_slope,
                    xx,
                    xy,
                    yx,
                    yy,
                );
                new_start_slope = r_slope;
            }
        }
        if blocked {
            break;
        }
    }
}

fn set_visible(visible: &mut [bool], x: i32, y: i32) {
    if x < 0 || y < 0 || x >= MAP_W as i32 || y >= MAP_H as i32 {
        return;
    }
    visible[(y as u32 * MAP_W + x as u32) as usize] = true;
}
