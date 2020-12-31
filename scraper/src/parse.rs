use std::str::FromStr;

use anyhow::Result;
use chrono::{NaiveTime, Weekday};
use select::{
    document::Document,
    node::Node,
    predicate::{Attr, Name, Predicate},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Serialize)]
pub struct Course {
    id: String,
    title: String,
    credits: u32,
}

#[derive(Serialize)]
pub struct Class {
    id: String,
    course: Course,
    labels: Vec<String>,
    capacity: u32,
    enrolled: u32,
    waiting: u32,
    times: Vec<Time>,
    professors: Vec<String>,
}

#[derive(Serialize)]
pub struct Time {
    weekday: Weekday,
    time: NaiveTime,
    credits: u32,
    place: String,
}

#[derive(Debug, Error)]
enum ParseError {
    #[error("<tbody id=\"formBusca:dataTable:tb\"> not found")]
    TableNotFound,
    #[error("invalid time")]
    InvalidTime,
    #[error("no course title")]
    NoCourseTitle,
}

impl FromStr for Time {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        fn split_once<'a>(s: &'a str, pat: &str) -> Result<(&'a str, &'a str)> {
            let mut parts = s.splitn(2, pat);
            match (parts.next(), parts.next()) {
                (Some(a), Some(b)) => Ok((a, b)),
                _ => Err(ParseError::InvalidTime.into()),
            }
        }

        let (time, place) = split_once(s, " / ")?;
        let place = place.to_owned();

        let (weekday, time) = split_once(time, ".")?;
        let weekday = match weekday.parse()? {
            1 => Weekday::Sun,
            2 => Weekday::Mon,
            3 => Weekday::Tue,
            4 => Weekday::Wed,
            5 => Weekday::Thu,
            6 => Weekday::Fri,
            7 => Weekday::Sat,
            _ => return Err(ParseError::InvalidTime.into()),
        };

        let (time, credits) = split_once(time, "-")?;
        let credits = credits.trim().parse()?;

        let time = NaiveTime::parse_from_str(time, "%H%M")?;

        Ok(Time {
            weekday,
            time,
            credits,
            place,
        })
    }
}

impl Class {
    fn from_html(row: &Node) -> Result<Self> {
        let fields = row
            .find(Name("td"))
            .map(|node| node.text().trim().to_owned())
            .collect::<Vec<_>>();

        let course_id = &fields[3];
        let class_id = &fields[4];

        let mut l = fields[5].lines().map(str::trim);
        let course_title = l.next().ok_or(ParseError::NoCourseTitle)?;

        let labels = l
            .map(|label| {
                let brackets = ['[', ']'];
                label.trim_matches(brackets.as_ref()).to_owned()
            })
            .collect();

        let credits = fields[6].parse()?;
        let capacity = fields[7].parse()?;
        let enrolled = fields[8].parse()?;
        let waiting = if fields[11].is_empty() {
            0
        } else {
            fields[11].parse()?
        };

        let course = Course {
            id: course_id.clone(),
            title: course_title.to_owned(),
            credits,
        };

        let times = fields[12]
            .lines()
            .map(str::trim)
            .map(str::parse)
            .collect::<Result<_>>()?;

        let professors = fields[13]
            .lines()
            .map(str::trim)
            .map(str::to_owned)
            .collect();

        Ok(Class {
            id: class_id.clone(),
            course,
            labels,
            capacity,
            enrolled,
            waiting,
            times,
            professors,
        })
    }
}

pub fn classes_from_html(source: &str) -> Result<Vec<Class>> {
    let document = Document::from(source);

    let table = document
        .find(Name("tbody").and(Attr("id", "formBusca:dataTable:tb")))
        .next()
        .ok_or(ParseError::TableNotFound)?;

    table
        .find(Name("tr"))
        .map(|row| Class::from_html(&row))
        .collect()
}
