use scraper::{ElementRef, Html};
use std::str::FromStr;

pub fn get_text_of_html_str(html: &str) -> String {
    Html::parse_fragment(&html)
        .root_element()
        .text()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn get_text_of_element(ele: ElementRef) -> String {
    ele.text().map(|t| t.trim()).collect::<Vec<_>>().concat()
}

pub fn get_text_arr_of_html_str(html: &str) -> Vec<String> {
    Html::parse_fragment(&html)
        .root_element()
        .text()
        .map(|t| t.trim().into())
        .collect::<Vec<_>>()
}

pub fn get_text_arr_of_children_element(ele: ElementRef) -> Vec<String> {
    ele.children()
        .into_iter()
        .map(|x| {
            if let Some(e) = ElementRef::wrap(x) {
                return e.text().map(|t| t.trim()).collect::<Vec<_>>().concat();
            }
            "".to_string()
        })
        .filter(|x| !x.is_empty())
        .collect()
}

//提取字符串中唯一的正整数
pub fn extract_integer<F: FromStr + Default>(s: &str) -> F {
    let l = s.find(|x: char| x.is_digit(10)).unwrap_or(s.len());
    if l == s.len() {
        return F::default();
    }
    let r = s.rfind(|x: char| x.is_digit(10)).unwrap_or(0) + 1;
    s[l..r].parse().unwrap_or_default()
}
