use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    cache::ReadImageToCache,
    result::{to_rv, RvResult},
    rverr,
    types::ResultImage,
};

#[derive(Clone)]
pub struct ReadImageFromPyHttp;

impl ReadImageToCache<()> for ReadImageFromPyHttp {
    fn new(_: ()) -> RvResult<Self> {
        Ok(Self {})
    }

    fn read(&self, url: &str) -> ResultImage {
        let resp = || reqwest::blocking::get(url)?.bytes();
        let image_byte_blob = resp().map_err(to_rv)?;
        image::load_from_memory(&image_byte_blob).map_err(to_rv)
    }

    fn ls(&self, address: &str) -> RvResult<Vec<String>> {
        lazy_static! {
            static ref LI_REGEX: Regex = Regex::new(r"<li>.*</li>").unwrap();
        }
        lazy_static! {
            static ref HREF_REGEX: Regex = Regex::new("href\\s*=\\s*\".*\"").unwrap();
        }
        println!("{}", address);
        let address = address.replace(' ', "%20");
        let resp = || reqwest::blocking::get(&address)?.text();
        let text = resp().map_err(to_rv)?;
        Ok(LI_REGEX
            .find_iter(&text)
            .flat_map(|found| {
                let li_text = &text[found.start()..found.end()];
                let found_href = HREF_REGEX.find(li_text);
                let len_href = "href=\"".len();
                found_href.map(|fh| {
                    format!(
                        "{}/{}",
                        address,
                        &li_text[(fh.start() + len_href)..(fh.end() - 1)]
                    )
                })
            })
            .collect())
    }
    fn file_info(&self, _: &str) -> RvResult<String> {
        Err(rverr!("http reader cannot read file info",))
    }
}
