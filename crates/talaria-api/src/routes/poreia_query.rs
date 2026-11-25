// crates/talaria-api/src/routes/poreia_query.rs
use chrono::NaiveDate;

pub const MAX_RADIUS_KM: f64 = 50.0;
pub const MAX_PAGE: i64 = 50;

#[derive(Debug, Clone, PartialEq)]
pub struct GeoWindow {
    pub lat: f64,
    pub lon: f64,
    pub radius_m: f64,
    pub limit: i64,
    pub offset: i64,
}

pub fn parse_geo(
    lat: Option<f64>,
    lon: Option<f64>,
    radius_km: Option<f64>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<GeoWindow, &'static str> {
    let lat = lat.ok_or("invalid_geo_query")?;
    let lon = lon.ok_or("invalid_geo_query")?;
    let radius_km = radius_km.ok_or("invalid_geo_query")?;
    if !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
        return Err("invalid_latitude");
    }
    if !lon.is_finite() || !(-180.0..=180.0).contains(&lon) {
        return Err("invalid_longitude");
    }
    if !radius_km.is_finite() || !(0.1..=MAX_RADIUS_KM).contains(&radius_km) {
        return Err("invalid_radius");
    }
    let limit = limit.unwrap_or(MAX_PAGE);
    let offset = offset.unwrap_or(0);
    if !(1..=MAX_PAGE).contains(&limit) {
        return Err("invalid_limit");
    }
    if !(0..=10_000).contains(&offset) {
        return Err("invalid_offset");
    }
    Ok(GeoWindow {
        lat,
        lon,
        radius_m: radius_km * 1_000.0,
        limit,
        offset,
    })
}

pub fn parse_lang(lang: Option<&str>) -> Result<&'static str, &'static str> {
    match lang {
        None | Some("fr") => Ok("fr"),
        Some("en") => Ok("en"),
        Some(_) => Err("invalid_lang"),
    }
}

pub fn parse_day(value: &str) -> Result<NaiveDate, &'static str> {
    if value.len() != 10 || value.contains('T') || value.contains(':') {
        return Err("invalid_date");
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "invalid_date")
}

pub fn parse_range(from: &str, to: &str) -> Result<(NaiveDate, NaiveDate), &'static str> {
    let start = parse_day(from)?;
    let end = parse_day(to)?;
    if end < start {
        return Err("invalid_date_range");
    }
    let days = (end - start).num_days();
    if days > 366 {
        return Err("date_range_too_wide");
    }
    Ok((start, end))
}

pub fn parse_bbox(value: &str) -> Result<(f64, f64, f64, f64), &'static str> {
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 4 {
        return Err("invalid_bbox");
    }
    let numbers = parts
        .iter()
        .map(|part| part.trim().parse::<f64>().map_err(|_| "invalid_bbox"))
        .collect::<Result<Vec<f64>, _>>()?;
    let (west, south, east, north) = (numbers[0], numbers[1], numbers[2], numbers[3]);
    if [west, south, east, north]
        .iter()
        .any(|number| !number.is_finite())
    {
        return Err("invalid_bbox");
    }
    if !(-180.0..=180.0).contains(&west)
        || !(-180.0..=180.0).contains(&east)
        || !(-90.0..=90.0).contains(&south)
        || !(-90.0..=90.0).contains(&north)
        || east < west
        || north < south
    {
        return Err("invalid_bbox");
    }
    Ok((west, south, east, north))
}
