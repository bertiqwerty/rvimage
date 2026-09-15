use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
    vec,
};

use crate::{
    cache::ReadImageToCache, image_reader::core::SUPPORTED_EXTENSIONS, types::ResultImage,
};
use azure_core::http::Url;
use azure_storage_blob::{BlobContainerClient, models::BlobContainerClientListBlobsOptions};
use futures::TryStreamExt;
use lazy_static::lazy_static;
use rvimage_domain::{RvResult, rverr, to_rv};
use std::sync::Arc;
use tokio::{runtime::Runtime, time::timeout};

lazy_static! {
    static ref RT: Runtime = Runtime::new().unwrap();
}

pub fn make_connection_string(
    cur_prj_path: &Path,
    connection_string_path: &str,
    connection_string: &str,
) -> RvResult<String> {
    if connection_string.trim().is_empty() {
        let csp = PathBuf::from(connection_string_path);
        let csp = if csp.is_absolute() {
            csp
        } else {
            cur_prj_path
                .parent()
                .expect("current project file cannot be in no parent directory")
                .join(csp)
        };
        let connection_string = fs::read_to_string(&csp).map_err(to_rv)?;
        let line_with_cs = connection_string.lines().find(|line| {
            let line = line.trim();
            !line.starts_with('#') && (line.to_lowercase().contains("connection_string"))
                || line.to_lowercase().contains("azure_connection_string")
        });
        Ok(if let Some(line_with_cs) = line_with_cs {
            line_with_cs
                .split_once('=')
                .map(|(_, cs)| cs.trim().to_string())
                .ok_or(rverr!(
                    "cannot parse connection string from line {:?}",
                    line_with_cs
                ))?
        } else {
            connection_string
        })
    } else {
        Ok(connection_string.to_string())
    }
}

#[derive(Clone)]
pub struct AzureConnectionData {
    pub connection_string: String,
    pub container_name: String,
    pub blob_list_timeout_s: u64,
}

async fn blob_list(
    container_client: &BlobContainerClient,
    prefix: &str,
    page_timeout_s: u64,
) -> RvResult<Vec<String>> {
    let mut res = vec![];
    let options = BlobContainerClientListBlobsOptions {
        prefix: (!prefix.is_empty()).then(|| prefix.to_string()),
        ..Default::default()
    };
    let mut stream = container_client.list_blobs(Some(options)).map_err(to_rv)?;
    while let Some(blob) = timeout(Duration::from_secs(page_timeout_s), stream.try_next())
        .await
        .map_err(|_| {
            rverr!("timeout while listing Azure blobs, waited more than {page_timeout_s} seconds")
        })?
        .map_err(to_rv)?
    {
        if let Some(name) = blob.name
            && SUPPORTED_EXTENSIONS.iter().any(|ext| name.ends_with(*ext))
        {
            res.push(name);
        }
        tracing::info!("retrieved {} blobs ", res.len());
    }
    Ok(res)
}

async fn download_blob(
    container_client: &BlobContainerClient,
    blob_name: &str,
) -> RvResult<Vec<u8>> {
    let blob_client = container_client.blob_client(blob_name);
    let response = blob_client.download(None).await.map_err(to_rv)?;
    let data = response.body.collect().await.map_err(to_rv)?;
    Ok(data.to_vec())
}

/// Build a container URL from either a full SAS URL or a SAS connection string
/// (`BlobEndpoint=...;SharedAccessSignature=...`) combined with the container name.
fn build_container_url(sas: &str, container_name: &str) -> RvResult<Url> {
    let sas = sas.trim();
    // A full SAS URL already points at the container and carries the token.
    if sas.starts_with("http://") || sas.starts_with("https://") {
        return Url::parse(sas).map_err(to_rv);
    }
    let mut blob_endpoint = None;
    let mut account_name = None;
    let mut endpoint_suffix = None;
    let mut protocol = None;
    let mut token = None;
    for part in sas.split(';') {
        // split_once('=') keeps '=' inside the SAS token intact.
        if let Some((key, value)) = part.split_once('=') {
            let value = value.trim();
            match key.trim().to_lowercase().as_str() {
                "blobendpoint" => blob_endpoint = Some(value.trim_end_matches('/').to_string()),
                "accountname" => account_name = Some(value.to_string()),
                "endpointsuffix" => endpoint_suffix = Some(value.to_string()),
                "defaultendpointsprotocol" => protocol = Some(value.to_string()),
                "sharedaccesssignature" => token = Some(value.trim_start_matches('?').to_string()),
                _ => {}
            }
        }
    }
    let endpoint = blob_endpoint.or_else(|| {
        Some(format!(
            "{}://{}.blob.{}",
            protocol.as_deref().unwrap_or("https"),
            account_name?,
            endpoint_suffix.as_deref().unwrap_or("core.windows.net"),
        ))
    });
    let endpoint = endpoint.ok_or_else(|| {
        rverr!("could not determine blob endpoint from Azure SAS connection string")
    })?;
    let token = token.ok_or_else(|| {
        rverr!("could not find SharedAccessSignature in Azure SAS connection string")
    })?;
    Url::parse(&format!("{endpoint}/{container_name}?{token}")).map_err(to_rv)
}

#[derive(Clone)]
pub struct ReadImageFromAzureBlob {
    container_client: Arc<BlobContainerClient>,
    page_timeout_s: u64,
}

impl ReadImageToCache<AzureConnectionData> for ReadImageFromAzureBlob {
    fn new(conn_data: AzureConnectionData) -> RvResult<Self> {
        let container_url =
            build_container_url(&conn_data.connection_string, &conn_data.container_name)?;
        let container_client =
            BlobContainerClient::new(container_url, None, None).map_err(to_rv)?;
        Ok(Self {
            container_client: Arc::new(container_client),
            page_timeout_s: conn_data.blob_list_timeout_s,
        })
    }

    fn read(&self, blob_name: &str) -> ResultImage {
        let blob = RT.block_on(download_blob(&self.container_client, blob_name))?;
        image::load_from_memory(&blob).map_err(to_rv)
    }

    fn ls(&self, prefix: &str) -> RvResult<Vec<String>> {
        RT.block_on(blob_list(
            &self.container_client,
            prefix,
            self.page_timeout_s,
        ))
    }

    fn file_info(&self, _: &str) -> RvResult<String> {
        Err(rverr!("cannot read file info from azure blob"))
    }
}
