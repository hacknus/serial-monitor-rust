use eframe::egui::{self, text::LayoutJob, Color32, TextFormat};
use eframe::egui::{FontFamily, FontId};

extern crate regex;
use regex::Regex;
use regex::RegexSet;
const DEFAULT_FONT_ID: FontId = FontId::new(14.0, FontFamily::Monospace);

#[derive(Debug, Clone, Copy)]
pub struct HighLightElement {
    pos_start: usize,
    pos_end: usize,
    token_idx: usize,
}
impl HighLightElement {
    pub fn new(pos_start: usize, pos_end: usize, token_idx: usize) -> Self {
        Self {
            pos_start,
            pos_end,
            token_idx,
        }
    }
}
pub fn highlight_impl(
    _ctx: &egui::Context,
    text: &str,
    tokens: Vec<String>,
    default_color: Color32,
) -> Option<LayoutJob> {
    // Extremely simple syntax highlighter for when we compile without syntect

    let mut my_tokens = tokens.clone();
    for token in my_tokens.clone() {
        if token.is_empty() {
            let index = my_tokens.iter().position(|x| *x == token).unwrap();
            my_tokens.remove(index);
        }
    }

    let content_string = String::from(text);
    // let _ = file.read_to_string(&mut isi);
    let mut regexs: Vec<Regex> = Vec::new();
    for sentence in my_tokens.clone() {
        match Regex::new(&sentence) {
            Ok(re) => {
                regexs.push(re);
            }
            Err(_err) => {}
        };
    }

    let mut highlight_list: Vec<HighLightElement> = Vec::<HighLightElement>::new();
    match RegexSet::new(my_tokens.clone()) {
        Ok(set) => {
            for idx in set.matches(&content_string).into_iter() {
                for caps in regexs[idx].captures_iter(&content_string) {
                    highlight_list.push(HighLightElement::new(
                        caps.get(0).unwrap().start(),
                        caps.get(0).unwrap().end(),
                        idx,
                    ));
                }
            }
        }
        Err(_err) => {}
    };

    highlight_list.sort_by_key(|item| (item.pos_start, item.pos_end));

    let mut job = LayoutJob::default();
    let mut previous = HighLightElement::new(0, 0, 0);
    for matches in highlight_list {
        if previous.pos_end >= matches.pos_start {
            continue;
        }
        job.append(
            &text[previous.pos_end..(matches.pos_start)],
            0.0,
            TextFormat::simple(DEFAULT_FONT_ID, default_color),
        );
        if matches.token_idx == 0 {
            job.append(
                &text[matches.pos_start..matches.pos_end],
                0.0,
                TextFormat::simple(DEFAULT_FONT_ID, Color32::from_rgb(255, 100, 100)),
            );
        } else if matches.token_idx == 1 {
            job.append(
                &text[matches.pos_start..matches.pos_end],
                0.0,
                TextFormat::simple(DEFAULT_FONT_ID, Color32::from_rgb(225, 159, 0)),
            );
        } else if matches.token_idx == 2 {
            job.append(
                &text[matches.pos_start..matches.pos_end],
                0.0,
                TextFormat::simple(DEFAULT_FONT_ID, Color32::from_rgb(87, 165, 171)),
            );
        } else if matches.token_idx == 3 {
            job.append(
                &text[matches.pos_start..matches.pos_end],
                0.0,
                TextFormat::simple(DEFAULT_FONT_ID, Color32::from_rgb(109, 147, 226)),
            );
        }
        previous = matches;
    }
    job.append(
        &text[previous.pos_end..],
        0.0,
        TextFormat::simple(DEFAULT_FONT_ID, default_color),
    );

    Some(job)
}
