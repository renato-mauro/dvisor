use crate::{err, error::DvisorResult};
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = globalThis, js_name = js_fetch_range, catch)]
    fn js_fetch_range(url: &str, start: u64, end: u64) -> Result<Uint8Array, JsValue>;

    #[wasm_bindgen(js_namespace = globalThis, js_name = js_get_content_length, catch)]
    fn js_get_content_length(url: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = globalThis, js_name = js_fetch_string, catch)]
    fn js_fetch_string(url: &str) -> Result<String, JsValue>;
}

pub struct URLRequest;

impl URLRequest {
    pub fn fetch_range(url: &str, offset: u64, size: usize) -> DvisorResult<Vec<u8>> {        
        let resp = js_fetch_range(url, offset, offset+(size as u64)-1)?;
        Ok(resp.to_vec())
    }
    
    pub fn resource_len(url: &str) -> DvisorResult<u64> {
        let len_str = js_get_content_length(url)?;
        if len_str.is_null() || len_str.is_undefined() {
            err!("value is null/undefined")
        } else {
            if let Some(s) = len_str.as_string() {
                s.parse::<u64>().map_err(|_| "String is not a valid u64".into())
            } else {
                err!("value is not a string")
            }
        }
    }

    pub fn fetch_string(url: &str) -> DvisorResult<String> {
        let resp = js_fetch_string(url)?;
        Ok(resp)
    }

}

// impl From<ureq::Error> for DvisorError {
//     fn from(e: ureq::Error) -> Self {
//         DvisorError::Msg(e.to_string())
//     }
// }

// impl URLRequest {
//     pub fn fetch_range(url: &str, offset: u64, size: usize) -> DvisorResult<Vec<u8>> {
//         let range = format!("bytes={}-{}", offset, offset+(size as u64)-1);
//         let resp = ureq::get(url)
//             .header("Range", &range)
//             .call()?
//         ;
//         let mut buf= Vec::with_capacity(size);
//         let mut reader = resp.into_body().into_reader();
//         reader.read_to_end(&mut buf)?;
//         Ok(buf)
//     }
    
//     pub fn resource_len(url: &str) -> DvisorResult<u64> {
//         let resp = ureq::head(url)
//             .header("Accept-Encoding", "identity") // size of the bytes you'd GET
//             .call()?
//         ;
//         if let Some(hv) = resp.headers().get("Content-Length") {
//             if let Ok(str_value) = hv.to_str() {
//                 if let Ok(value) = str_value.trim().parse::<u64>() {
//                     return Ok(value);
//                 } else {
//                     throw!("Error in readling Content-Length: '{}'", str_value);
//                 }
//             } else {
//                 throw!("Error in readling Content-Length");
//             }
//         } else {
//             throw!("Content-Length not sent in response header");
//         }
//     }

//     pub fn fetch_string(url: &str) -> DvisorResult<String> {
//         let resp = ureq::get(url)
//             .call()?
//         ;
//         Ok(resp.into_body().read_to_string()?)
//     }

// }

