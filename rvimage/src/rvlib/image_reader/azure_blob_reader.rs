use std::{fs, path::PathBuf, time::Duration, vec};

use crate::{
    cache::ReadImageToCache, image_reader::core::SUPPORTED_EXTENSIONS, types::ResultImage,
};
use azure_storage::ConnectionString;
use azure_storage_blobs::prelude::*;
use futures::StreamExt;
use lazy_static::lazy_static;
use rvimage_domain::{RvResult, rverr, to_rv};
use tokio::{runtime::Runtime, time::timeout};

lazy_static! {
    static ref RT: Runtime = Runtime::new().unwrap();
}

#[derive(Clone)]
pub struct AzureConnectionData {
    pub current_prj_path: PathBuf,
    pub connection_string_path: PathBuf,
    pub container_name: String,
    pub blob_list_timeout_s: u64,
}

async fn blob_list(
    container_client: &ContainerClient,
    prefix: &str,
    page_timeout_s: u64,
) -> RvResult<Vec<String>> {
    let mut res = vec![];
    let mut stream = if prefix.is_empty() {
        container_client.list_blobs().into_stream()
    } else {
        container_client
            .list_blobs()
            .prefix(prefix.to_string())
            .into_stream()
    };
    let container_name = container_client.container_name();
    while let Some(value) = timeout(Duration::from_secs(page_timeout_s), stream.next())
        .await
        .map_err(|_| {
            rverr!("timeout while listing Azure blobs of container {container_name}, waited more than {page_timeout_s} seconds; error: tokio::time::Elapased")
        })?
    {
        let page = value.map_err(|e| {
            rverr!(
                "could not list blobs for container '{}' due to '{:?}'",
                container_client.container_name(),
                e
            )
        })?;
        for cont in page.blobs.blobs().filter(|b| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|ext| *ext == &b.name[(b.name.len() - ext.len())..b.name.len()])
        }) {
            res.push(cont.name.clone());
        }
        tracing::info!("retrieved {} blobs ", res.len());
    }
    Ok(res)
}

async fn download_blob(container_client: &ContainerClient, blob_name: &str) -> RvResult<Vec<u8>> {
    let blob_client = container_client.blob_client(blob_name);
    blob_client.get_content().await.map_err(to_rv)
}

#[derive(Clone)]
pub struct ReadImageFromAzureBlob {
    container_client: ContainerClient,
    page_timeout_s: u64,
}

impl ReadImageToCache<AzureConnectionData> for ReadImageFromAzureBlob {
    fn new(conn_data: AzureConnectionData) -> RvResult<Self> {
        let constr_path = if conn_data.connection_string_path.is_absolute() {
            conn_data.connection_string_path
        } else {
            conn_data
                .current_prj_path
                .parent()
                .expect("current project file cannot be in no parent directory")
                .join(conn_data.connection_string_path)
        };
        let connection_string = fs::read_to_string(&constr_path).map_err(to_rv)?;
        let line_with_cs = connection_string.lines().find(|line| {
            !line.starts_with('#') && (line.to_lowercase().contains("connection_string"))
                || line.to_lowercase().contains("azure_connection_string")
        });
        let connection_string = if let Some(line_with_cs) = line_with_cs {
            line_with_cs
                .split_once('=')
                .map(|(_, cs)| cs.trim().to_string())
                .ok_or(rverr!(
                    "cannot parse connection string from line {:?}",
                    line_with_cs
                ))?
        } else {
            connection_string
        };

        let connection_string = ConnectionString::new(&connection_string).map_err(to_rv)?;
        let blob_service_client = BlobServiceClient::new(
            connection_string.account_name.unwrap(),
            connection_string.storage_credentials().map_err(to_rv)?,
        );
        let container_client = blob_service_client.container_client(conn_data.container_name);
        Ok(Self {
            container_client,
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
