use serde_json::Value;

/// Extract (x, y) series from a /api/ds/query Prometheus frames response.
/// Concatenates points from every frame so multi-series queries aren't truncated.
pub fn extract_series(resp: &Value) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = Vec::new();
    let results = match resp.get("results").and_then(|v| v.as_object()) {
        Some(r) => r,
        None => return out,
    };
    for (_ref, payload) in results {
        let frames = match payload.get("frames").and_then(|v| v.as_array()) {
            Some(f) => f,
            None => continue,
        };
        for frame in frames {
            let values = frame
                .get("data")
                .and_then(|v| v.get("values"))
                .and_then(|v| v.as_array());
            let Some(values) = values else { continue };
            if values.len() < 2 {
                continue;
            }
            let times = values[0].as_array();
            let nums = values[1].as_array();
            if let (Some(times), Some(nums)) = (times, nums) {
                for (t, y) in times.iter().zip(nums.iter()) {
                    if let (Some(t), Some(y)) = (t.as_f64(), y.as_f64()) {
                        if y.is_finite() {
                            out.push((t as f32 / 1000.0, y as f32));
                        }
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}

pub fn render_ascii(series: &[(f32, f32)], width: usize, height: usize) -> String {
    if series.is_empty() {
        return "(no data)".to_string();
    }
    // Per-timestamp aggregation: min / mean / max so multi-series queries
    // (e.g. one line per broker) show the spread rather than a single mean.
    let mut buckets: std::collections::BTreeMap<i64, (f64, u32, f32, f32)> =
        std::collections::BTreeMap::new();
    for (x, y) in series {
        let key = x.round() as i64;
        let e = buckets
            .entry(key)
            .or_insert((0.0, 0, f32::INFINITY, f32::NEG_INFINITY));
        e.0 += *y as f64;
        e.1 += 1;
        if *y < e.2 {
            e.2 = *y;
        }
        if *y > e.3 {
            e.3 = *y;
        }
    }
    let pts: Vec<(f32, f32, f32, f32)> = buckets
        .into_iter()
        .map(|(x, (sum, n, lo, hi))| (x as f32, (sum / n as f64) as f32, lo, hi))
        .collect();
    let series_count = pts.len(); // ts count
    let max_per_ts = series
        .iter()
        .fold(std::collections::BTreeMap::<i64, u32>::new(), |mut m, (x, _)| {
            *m.entry(x.round() as i64).or_insert(0) += 1;
            m
        })
        .values()
        .copied()
        .max()
        .unwrap_or(1);

    let xmin = pts.first().map(|p| p.0).unwrap_or(0.0);
    let xmax = pts.last().map(|p| p.0).unwrap_or(xmin + 1.0);
    let ymin_all = pts.iter().map(|p| p.2).fold(f32::INFINITY, f32::min);
    let ymax_all = pts.iter().map(|p| p.3).fold(f32::NEG_INFINITY, f32::max);
    let ymin = ymin_all;
    let ymax = if (ymax_all - ymin).abs() < 1e-9 {
        ymin + 1.0
    } else {
        ymax_all
    };
    let xspan = (xmax - xmin).max(1.0);
    let yspan = ymax - ymin;

    let w = width.max(20);
    let h = height.max(5);
    let mut grid: Vec<Vec<char>> = vec![vec![' '; w]; h];

    let y_to_row = |y: f32| -> usize {
        let cy = ((1.0 - (y - ymin) / yspan) * (h as f32 - 1.0)).round() as i32;
        cy.clamp(0, h as i32 - 1) as usize
    };
    let x_to_col = |x: f32| -> usize {
        let cx = (((x - xmin) / xspan) * (w as f32 - 1.0)).round() as i32;
        cx.clamp(0, w as i32 - 1) as usize
    };

    // Draw min-max band first (vertical strokes), then overlay mean line.
    for (x, _mean, lo, hi) in &pts {
        let cx = x_to_col(*x);
        let r_hi = y_to_row(*hi);
        let r_lo = y_to_row(*lo);
        let (a, b) = (r_hi.min(r_lo), r_hi.max(r_lo));
        for r in a..=b {
            if grid[r][cx] == ' ' {
                grid[r][cx] = '│';
            }
        }
        grid[r_hi][cx] = '▴';
        grid[r_lo][cx] = '▾';
    }
    let mut prev: Option<(usize, usize)> = None;
    for (x, mean, _lo, _hi) in &pts {
        let cx = x_to_col(*x);
        let cy = y_to_row(*mean);
        if let Some((px, py)) = prev {
            // simple sloped fill
            let dx = cx as i32 - px as i32;
            let dy = cy as i32 - py as i32;
            let steps = dx.abs().max(dy.abs()).max(1);
            for s in 1..=steps {
                let xi = px as i32 + dx * s / steps;
                let yi = py as i32 + dy * s / steps;
                if xi >= 0 && (xi as usize) < w && yi >= 0 && (yi as usize) < h {
                    let cell = &mut grid[yi as usize][xi as usize];
                    if *cell == ' ' || *cell == '│' {
                        *cell = '─';
                    }
                }
            }
        }
        grid[cy][cx] = '●';
        prev = Some((cx, cy));
    }

    let label_w = 10;
    let mut out = String::new();
    for (row, line) in grid.iter().enumerate() {
        let yval = ymax - (row as f32 / (h as f32 - 1.0)) * yspan;
        let label = if row == 0 || row == h - 1 || row == h / 2 {
            format!("{:>label_w$.2} │", yval, label_w = label_w)
        } else {
            format!("{:>label_w$} │", "", label_w = label_w)
        };
        out.push_str(&label);
        for c in line {
            out.push(*c);
        }
        out.push('\n');
    }
    out.push_str(&" ".repeat(label_w + 2));
    out.push_str(&"─".repeat(w));
    out.push('\n');
    let t0 = chrono::DateTime::<chrono::Utc>::from_timestamp(xmin as i64, 0)
        .map(|d| d.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| format!("{xmin:.0}"));
    let t1 = chrono::DateTime::<chrono::Utc>::from_timestamp(xmax as i64, 0)
        .map(|d| d.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| format!("{xmax:.0}"));
    out.push_str(&" ".repeat(label_w + 2));
    out.push_str(&format!("{:<width$}{}", t0, t1, width = w - t1.len()));
    out.push('\n');
    out.push_str(&format!(
        "y:[{ymin_all:.4}, {ymax_all:.4}]   raw={}   timestamps={}   max series/ts={}   ▴=max ●=mean ▾=min",
        series.len(),
        series_count,
        max_per_ts
    ));
    out
}
