use regex::Regex;
use crate::{driver::parquetdriver::pqdatasource::ParquetDataSource, err, error::DvisorResult, model::{datasourcewithmetadata::DataSourceWithMetadata, dvisordatasource::IDvisorDataSource, metadata::Metadata}, urlrequest::URLRequest};

pub struct Requester {
}

impl Requester {

    pub fn new() -> Requester {
        Requester { }
    }

    pub fn create_metadata(&self, name: &str) -> DvisorResult<Metadata> {
        // A `blob:` URL (created client-side from a local File via a
        // <input type=file> upload, never typed by a user) has no
        // `scheme://host/path.ext` shape at all — it's `blob:<origin>/<uuid>`,
        // so it can never match try_parse_url's http(s)+extension regex.
        // The upload UI already guarantees the file is a .parquet before
        // handing dvisor a blob: URL, so extension-sniffing is skipped
        // entirely for this scheme rather than trying to make the regex fit.
        if name.starts_with("blob:") {
            let ds = open_parquet_from_url(name)?;
            return Ok(Metadata::new(&ds)?);
        }
        if let Some((_, _, ext)) = try_parse_url(name) {
            if ext.eq_ignore_ascii_case("parquet") {
                let ds = open_parquet_from_url(name)?;
                Ok(Metadata::new(&ds)?)
            }
            else {
                err!("Unsuported file extension for data table: {}. Use parquet instead.",ext)
            }
        } else {
            err!("Invalid URL: {}", name)
        }
    }

    pub fn create_data_source_with_metadata(&self, name: &str) -> DvisorResult<DataSourceWithMetadata> {
        // Same reasoning as create_metadata above: a blob: URL has no
        // scheme://host/path.ext shape to match, and — being a bare opaque
        // UUID — no "sidecar .meta.json next to it" concept either, so
        // there is no viability-guard metadata to look for; querying
        // proceeds without it (same as any remote file missing its
        // .meta.json today), just without row-group pruning.
        if name.starts_with("blob:") {
            let ds = open_parquet_from_url(name)?;
            return Ok(DataSourceWithMetadata::new(ds, None));
        }
        if let Some((base, file, ext)) = try_parse_url(name) {
            if ext.eq_ignore_ascii_case("parquet") {
                let metadata_file_name = format!("{}{}{}",base,file,".meta.json");
                let ds = open_parquet_from_url(name)?;
                let metadata = read_metadata_from_url(&metadata_file_name)?;
                // Se metadata for None, pela ausência do arquivo,
                // carregar as estatísticas do próprio arquivo parquet.
                // Usar o modelo do proprio create metadata, mas ser ler 
                // o arquivo para calcular maximos e minimos. 
                // criar uma versão "light"
                Ok(DataSourceWithMetadata::new(ds,metadata))
            }
            else {
                err!("Unsuported file extension for data table: {}. Use parquet instead.",ext)
            }
        } else {
            err!("Invalid URL: {}", name)
        }
    }
}

fn try_parse_url(url: &str) -> Option<(String, String, String)> {
    let regex = Regex::new(r"(?P<baseurl>^http[s]?://.*/)(?:(?P<filename>.*)(?:\.(?P<ext>.*)))$").unwrap();
    let caps = regex.captures(url)?;
    let baseurl = caps.name("baseurl").map(|m|m.as_str().to_string())?;
    let filename = caps.name("filename").map(|m|m.as_str().to_string())?;
    let ext = caps.name("ext").map(|m|m.as_str().to_string())?;
    Some((baseurl,filename,ext))
}

pub fn open_parquet_from_url(url: &str) -> DvisorResult<Box<dyn IDvisorDataSource>> {
    Ok(Box::new(ParquetDataSource::new(url)?))
}

pub fn read_metadata_from_url(url: &str) -> DvisorResult<Option<Metadata>> {
    let buf = URLRequest::fetch_string(url)?;
    Ok(Some(Metadata::from_json(&buf)?))
}
