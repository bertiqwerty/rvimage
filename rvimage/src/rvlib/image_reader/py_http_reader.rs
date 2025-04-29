use std::time::Duration;

use crate::{cache::ReadImageToCache, file_util, types::ResultImage};
use lazy_static::lazy_static;
use regex::Regex;
use rvimage_domain::{rverr, to_rv, RvResult};

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
        let address = file_util::url_encode(address);
        let c = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build();
        let resp = c.and_then(|c| {
            let request = c.get(&address).build();
            request.and_then(|r| c.execute(r))
        });
        let text = resp
            .and_then(|r| r.text())
            .map_err(|e| rverr!("pyhttp reader cannot read {address} due to {e:?}"))?;
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
                        file_util::url_encode(&li_text[(fh.start() + len_href)..(fh.end() - 1)])
                    )
                })
            })
            .collect())
    }
    fn file_info(&self, _: &str) -> RvResult<String> {
        Err(rverr!("http reader cannot read file info"))
    }
}
