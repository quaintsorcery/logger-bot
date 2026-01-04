use std::io::Cursor;

use anyhow::Context;
use chrono::{DateTime, Datelike, Timelike, Utc};
use plotters::prelude::*;

use image::{ImageBuffer, Rgb};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

pub fn generate_personal_annual_chart(
    username: &str,
    timestamps: Vec<i64>,
    year: Option<i32>,
) -> anyhow::Result<Vec<u8>> {
    let mut buffer = vec![0u8; (WIDTH * HEIGHT * 3) as usize];
    let year = match year {
        Some(y) => y,
        None => Utc::now().year(),
    };
    let data = prepare_annual_data(timestamps, year);
    draw_chart(
        ChartParams {
            caption: &format!("{username} - {year}"),
            x_desc: "Month",
            y_desc: "Score",
        },
        &data,
        &mut buffer,
    )?;
    Ok(make_png(buffer)?)
}

pub fn generate_personal_hourly_chart(
    username: &str,
    timestamps: Vec<i64>,
) -> anyhow::Result<Vec<u8>> {
    let mut buffer = vec![0u8; (WIDTH * HEIGHT * 3) as usize];
    let data = prepare_hourly_data(timestamps);
    draw_chart(
        ChartParams {
            caption: &username,
            x_desc: "Hour, UTC",
            y_desc: "Score",
        },
        &data,
        &mut buffer,
    )?;
    Ok(make_png(buffer)?)
}

fn make_png(buffer: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let image: ImageBuffer<Rgb<u8>, _> =
        ImageBuffer::from_raw(WIDTH, HEIGHT, buffer).context("Failed to create an image buffer")?;
    let mut png_bytes = Vec::new();
    let mut cursor = Cursor::new(&mut png_bytes);
    image::DynamicImage::ImageRgb8(image).write_to(&mut cursor, image::ImageFormat::Png)?;
    Ok(png_bytes)
}

fn prepare_annual_data(timestamps: Vec<i64>, year: i32) -> [ChartData; 12] {
    timestamps
        .iter()
        .filter_map(|&ts| DateTime::from_timestamp(ts, 0))
        .filter(|dt| dt.year() == year)
        .fold([0usize; 12], |mut acc, dt| {
            acc[(dt.month() - 1) as usize] += 1;
            acc
        })
        .map(|v| ChartData {
            value: v,
            label: None,
        })
}

fn prepare_hourly_data(timestamps: Vec<i64>) -> [ChartData; 24] {
    timestamps
        .iter()
        .filter_map(|&ts| DateTime::from_timestamp(ts, 0))
        .fold([0usize; 24], |mut acc, dt| {
            acc[dt.hour() as usize] += 1;
            acc
        })
        .map(|v| ChartData {
            value: v,
            label: None,
        })
}

struct ChartParams<'a> {
    caption: &'a str,
    x_desc: &'a str,
    y_desc: &'a str,
}

#[derive(Debug)]
struct ChartData {
    value: usize,
    label: Option<String>,
}

fn draw_chart(
    params: ChartParams,
    data: &[ChartData],
    mut buffer: &mut Vec<u8>,
) -> anyhow::Result<()> {
    let root = BitMapBackend::with_buffer(&mut buffer, (WIDTH, HEIGHT)).into_drawing_area();
    root.fill(&BLACK)?;

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .caption(params.caption, ("sans-serif", 30).into_font().color(&WHITE))
        .x_label_area_size(50)
        .y_label_area_size(50)
        .build_cartesian_2d(
            0..data.len(),
            0..(data.iter().map(|d| d.value).max().unwrap_or(1)),
        )?;

    chart
        .configure_mesh()
        .axis_style(WHITE.filled())
        .axis_desc_style(("sans-serif", 15).into_font().color(&WHITE))
        .x_desc(params.x_desc)
        .y_desc(params.y_desc)
        .label_style(("sans-serif", 15).into_font().color(&WHITE))
        .x_labels(data.len())
        .x_label_formatter(&|i| {
            data.get(*i)
                .and_then(|d| d.label.clone())
                .or_else(|| Some(format!("{i}")))
                .unwrap()
        })
        .draw()?;

    chart.draw_series(
        Histogram::vertical(&chart)
            .style(WHITE.filled())
            .data(data.iter().enumerate().map(|(i, d)| (i, d.value))),
    )?;

    root.present()?;
    Ok(())
}
