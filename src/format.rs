//! Numeric value formatting that reproduces the original decompiler output.

/// Format a value with three decimal places, matching the `0.000` format used
/// by the reference tool, including its handling of non finite values and
/// negative zero.
pub fn dec3(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    let text = format!("{:.3}", value);
    if text == "-0.000" {
        "0.000".to_string()
    } else {
        text
    }
}

/// Format a single precision float through the same path as [`dec3`].
pub fn dec3_f32(value: f32) -> String {
    dec3(value as f64)
}

/// Render a packed latitude/longitude/altitude triple.
///
/// Latitude and longitude arrive as 64 bit fixed point fractions of a full
/// revolution; altitude arrives as a 32.32 fixed point value in metres that is
/// converted to feet. The output format matches the simulator's `D2` notation,
/// for example `N0° 0' 0.00",E0° 0' 0.00",+000000.00`.
pub fn lla(lat_raw: i64, lon_raw: i64, alt_frac: u32, alt_whole: i32) -> String {
    let lat = lat_raw as f64 * 90.0 / 42_957_189_152_768_000.0;
    let lon = lon_raw as f64 * 360.0 / 18_446_744_073_709_552_000.0;
    let alt = (alt_whole as f64 + alt_frac as f64 / 4_294_967_296.0) * 3.280_839_9;

    let sign = if alt < 0.0 { '-' } else { '+' };
    format!(
        "{},{},{}{:09.2}",
        sexagesimal(lat, 'N', 'S'),
        sexagesimal(lon, 'E', 'W'),
        sign,
        alt.abs(),
    )
}

fn sexagesimal(value: f64, positive: char, negative: char) -> String {
    let prefix = if value < 0.0 { negative } else { positive };
    let abs = value.abs();
    let degrees = abs.floor();
    let minutes_full = (abs - degrees) * 60.0;
    let minutes = minutes_full.floor();
    let seconds = (minutes_full - minutes) * 60.0;
    format!(
        "{}{}\u{00b0} {}' {:.2}\"",
        prefix, degrees as i64, minutes as i64, seconds
    )
}
